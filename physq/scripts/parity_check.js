#!/usr/bin/env node
// =============================================================
// physq ranking-parity harness.
//
// Extracts the REAL ranking functions from ../../search.html (by function
// name, robust to edits elsewhere in the file) and runs them on the exact
// vectors physq's Rust unit tests assert. If the web algorithm's constants
// or formulas ever change, this fails — telling you the Rust port and its
// tests need the same change.
//
//   node physq/scripts/parity_check.js
// =============================================================
const fs = require('fs');
const path = require('path');

const html = fs.readFileSync(path.join(__dirname, '..', '..', 'search.html'), 'utf-8');

// Pull `function <name>(...) {...}` out of the page via brace matching.
function extract(name) {
  const start = html.indexOf(`function ${name}(`);
  if (start < 0) throw new Error(`function ${name} not found in search.html`);
  let i = html.indexOf('{', start);
  let depth = 0;
  for (; i < html.length; i++) {
    if (html[i] === '{') depth++;
    else if (html[i] === '}' && --depth === 0) break;
  }
  return html.slice(start, i + 1);
}

const NAMES = [
  'normalizeId',
  '_levenshtein',
  '_typoScore',
  '_ngramScore',
  '_expandQuery',
  '_buildBM25Index',
  '_bm25',
  '_getCandidates',
  '_scoreItem',
  '_rrfMerge',
];
// eslint-disable-next-line no-eval
const W = eval(`(() => { ${NAMES.map(extract).join('\n')}
  return { ${NAMES.join(', ')} }; })()`);

let failures = 0;
function check(name, got, expected, eps = 1e-9) {
  const ok =
    typeof expected === 'number'
      ? Math.abs(got - expected) < eps
      : JSON.stringify(got) === JSON.stringify(expected);
  if (!ok) failures++;
  console.log(
    `${ok ? 'PASS' : 'FAIL'} ${name}: js=${JSON.stringify(got)} rust_expects=${JSON.stringify(expected)}`
  );
}

// ── normalizeId (Rust: model::tests) ────────────────────────────────
check('normalizeId 00001', W.normalizeId('00001'), '1');
check('normalizeId 000', W.normalizeId('000'), '0');
check('normalizeId 00100', W.normalizeId('00100'), '100');
check(
  'normalizeId uuid',
  W.normalizeId('2e7f2483-54ac-4c28-9b19-e3f2e58fdc04'),
  '2e7f2483-54ac-4c28-9b19-e3f2e58fdc04'
);
check('normalizeId 1e3', W.normalizeId('1e3'), '1e3');
check('normalizeId " 007 "', W.normalizeId(' 007 '), '7');
check('normalizeId number 7', W.normalizeId(7), '7');

// ── levenshtein / typo / ngram (Rust: bm25::tests) ──────────────────
check('lev kitten/sitting', W._levenshtein('kitten', 'sitting'), 3);
check('lev 電磁/電波', W._levenshtein('電磁', '電波'), 1);
check('typo cat vs "bat hat"', W._typoScore('cat', 'bat hat'), 2);
check('typo ab vs "abcd"', W._typoScore('ab', 'abcd'), 1);
check('typo ab vs "xyzzy"', W._typoScore('ab', 'xyzzy'), 0);
check('typo exact-match zeroes', W._typoScore('cat', 'cat cut'), 0);
check('ngram abc in "xx abc yy"', W._ngramScore('abc', 'xx abc yy'), 1.0);
check('ngram abcd in "xxbcxx"', W._ngramScore('abcd', 'xxbcxx'), 0.5);
check('ngram 1-char word', W._ngramScore('a', 'aaaa'), 0);
check('ngram 電磁誘導 in "電磁 と 誘導"', W._ngramScore('電磁誘導', '電磁 と 誘導'), 1.0);

// ── BM25 on the tiny 3-doc corpus (Rust: bm25_matches_hand_computed) ─
const corpus3 = [
  { id: 'a', questions: [], search_text: '電磁 誘導 法則', priority: 1 },
  { id: 'b', questions: [], search_text: '電磁 電磁 波', priority: 1 },
  { id: 'c', questions: [], search_text: '運動 方程式', priority: 1 },
];
const idx = W._buildBM25Index(corpus3);
check('avgdl', idx.avgdl, 8 / 3);
const idf2 = Math.log((3 - 2 + 0.5) / (2 + 0.5) + 1);
const tfnA = (1 * 2.2) / (1 + 1.2 * (0.25 + (0.75 * 3) / (8 / 3)));
check('bm25 電磁 doc a', W._bm25('電磁', 'a', 3, idx), idf2 * tfnA);
const tfnB = (2 * 2.2) / (2 + 1.2 * (0.25 + (0.75 * 3) / (8 / 3)));
check('bm25 電磁 doc b', W._bm25('電磁', 'b', 3, idx), idf2 * tfnB);
const idf1 = Math.log((3 - 1 + 0.5) / (1 + 0.5) + 1);
const tfnC = (1 * 2.2) / (1 + 1.2 * (0.25 + (0.75 * 2) / (8 / 3)));
check('bm25 運動 doc c', W._bm25('運動', 'c', 2, idx), idf1 * tfnC);
check('bm25 unknown term', W._bm25('光', 'a', 3, idx), 0);

// ── _scoreItem field boosts (Rust: score_item_applies_field_boosts…) ─
{
  const items = [
    {
      id: 'a',
      questions: ['電磁誘導とは？'],
      keywords: ['電磁誘導'],
      synonyms: ['でんじゆうどう'],
      search_text: '電磁 誘導',
      priority: 0,
    },
    { id: 'b', questions: [], search_text: '運動 方程式', priority: 0 },
  ];
  const bidx = W._buildBM25Index(items);
  const q = '電磁誘導とは？';
  const s = W._scoreItem(items[0], [q], ['電磁'], q, bidx);
  const bm = W._bm25('電磁', 'a', 2, bidx);
  check('scoreItem exact-question case', s, bm + 10 + 3 + 1.0);

  const items2 = [{ id: 'a', questions: ['電磁誘導とは？'], search_text: '電磁 誘導', priority: 2 }];
  const bidx2 = W._buildBM25Index(items2);
  const s2 = W._scoreItem(items2[0], [q], ['電磁'], q, bidx2);
  const bm2 = W._bm25('電磁', 'a', 2, bidx2);
  check('scoreItem priority x2', s2, 2 * (bm2 + 10 + 3 + 1.0));
}

// ── keyword/synonym per-entry boosts (Rust: keyword_and_synonym_boosts…) ─
{
  const items = [
    {
      id: 'a',
      questions: [],
      keywords: ['電磁気学', '電磁波'],
      synonyms: ['電磁誘導'],
      search_text: 'zzz',
      priority: 0,
    },
  ];
  const bidx = W._buildBM25Index(items);
  const s = W._scoreItem(items[0], ['電磁'], ['電磁'], '電磁', bidx);
  check('scoreItem keyword+synonym', s, 3.0);
}

// ── adjacent pair boost (Rust: adjacent_pair_boost…) ────────────────
{
  const items = [{ id: 'a', questions: [], search_text: '電磁誘導 の 法則', priority: 0 }];
  const bidx = W._buildBM25Index(items);
  const s = W._scoreItem(items[0], ['電磁', '誘導'], [], '電磁 誘導', bidx);
  check('scoreItem adjacent pair', s, 2.0 + 0.5 + 0.5 + 1.0 + 1.0);
}

// ── RRF (Rust: rank::tests) ─────────────────────────────────────────
{
  const bm25Results = [{ id: 'A' }, { id: 'B' }, { id: 'C' }];
  const semanticRanked = [{ id: 'B' }, { id: 'D' }];
  const merged = W._rrfMerge(bm25Results, semanticRanked);
  const get = (id) => merged.find((m) => m.id === id).rrfScore;
  check('rrf A', get('A'), 1 / 61);
  check('rrf B', get('B'), 1 / 62 + 2 / 61);
  check('rrf C', get('C'), 1 / 63);
  check('rrf D', get('D'), 2 / 62);
  check(
    'rrf order',
    merged.map((m) => m.id),
    ['B', 'D', 'A', 'C']
  );
}

// ── _expandQuery: hira/kata + CJK bigrams, NO synonym expansion ─────
// (the CLI keeps hira/kata, swaps bigrams for lindera morphemes — §6)
{
  const { words, expanded } = W._expandQuery('電磁誘導 ぶつり');
  check('expand words', words, ['電磁誘導', 'ぶつり']);
  check('expand has katakana variant', expanded.includes('ブツリ'), true);
  check('expand has CJK bigram (web only — CLI drops)', expanded.includes('磁誘'), true);
}

console.log(failures === 0 ? '\nALL PARITY CHECKS PASSED' : `\n${failures} PARITY FAILURES`);
process.exit(failures === 0 ? 0 : 1);
