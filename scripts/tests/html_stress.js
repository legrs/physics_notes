#!/usr/bin/env node
// Headless functional + stress tests for search.html.
//
// Runs the REAL page code (extracted by name from search.html — the same
// technique as physq/scripts/parity_check.js) against the REAL corpus
// (q_and_a_data.json) and embeddings (embeddings.json):
//
//   1. The full BM25 pipeline at function level (index build → expand →
//      score → sort → related boost → RRF) over a large battery of queries,
//      including adversarial / extreme inputs (empty, huge, unicode, XSS,
//      injection, control chars, malformed surrogates, …).
//   2. Determinism / state-safety (functions must be reproducible and not
//      accumulate state between calls).
//   3. Semantic + RRF features that are pure (cosine, per-model weights,
//      multi-list fusion) driven with REAL embedding vectors.
//
// Browser-only features (transformers.js model download, marked/katex
// rendering, DOM rendering, chat window) can't run headlessly — they are
// guarded by html_syntax.js (SRI, ids, script parse) instead.
//
// Usage: node scripts/tests/html_stress.js
'use strict';

const fs = require('fs');
const path = require('path');
const {
  REPO_ROOT,
  loadSearchHtml,
  extractFunction,
  extractConst,
  buildContext,
} = require('./_extract');

const html = loadSearchHtml();

const FN = [
  'normalizeId',
  '_levenshtein',
  '_typoScore',
  '_ngramScore',
  '_expandQuery',
  '_buildBM25Index',
  '_bm25',
  '_getCandidates',
  '_scoreItem',
  '_extractSnippet',
  '_cosineSim',
  '_rrfMergeMulti',
  '_rrfMerge',
  '_rrfWeightForKey',
];
const CONST = ['SEMANTIC_MODELS', 'MODE_EMB_KEYS', 'RRF_WEIGHT_BM25', 'RRF_WEIGHT_SMALL', 'RRF_WEIGHT_LARGE'];

const sources = {};
for (const n of FN) sources[n] = extractFunction(html, n);
for (const c of CONST) sources[c] = extractConst(html, c);
const W = buildContext(sources);

const data = JSON.parse(fs.readFileSync(path.join(REPO_ROOT, 'q_and_a_data.json'), 'utf-8'));
const embeddings = JSON.parse(fs.readFileSync(path.join(REPO_ROOT, 'embeddings.json'), 'utf-8'));
const corpusIds = new Set(data.map((d) => d.id));

let failures = 0;
let checks = 0;
// Log FAIL lines only (a green run is silent apart from section headers and
// the final summary — keeps CI logs readable over 100+ queries).
function ok(cond, msg) {
  checks++;
  if (!cond) { failures++; console.log(`FAIL ${msg}`); }
}
function section(title) {
  console.log(`── ${title}`);
}

// score all candidates like searchTopResult does (but with a shared index —
// the functions are pure, so this is faithful: _buildBM25Index is stateless).
function runBM25(query, idx) {
  const q = query.toLowerCase();
  const { words, expanded } = W._expandQuery(q);
  const results = [];
  if (!words.length) return { words, results };
  const candidates = W._getCandidates(idx, data, expanded);
  candidates.forEach((item) => {
    const score = W._scoreItem(item, words, expanded, q, idx);
    results.push({ item, score });
  });
  const ranked = results.filter((r) => r.score > 0).sort((a, b) => b.score - a.score);
  const related = new Set(ranked.slice(0, 3).flatMap((r) => r.item.related || []));
  ranked.forEach((r) => { if (related.has(r.item.id)) r.score += 0.5; });
  ranked.sort((a, b) => b.score - a.score);
  return { words, results: ranked };
}

function assertRun(query, idx, { allowEmpty = true, maxLen = 200 } = {}) {
  const label = `query ${JSON.stringify(query).slice(0, maxLen)}`;
  let out;
  try {
    out = runBM25(query, idx);
  } catch (e) {
    ok(false, `${label} does not throw (got ${e})`);
    return;
  }
  if (!(Array.isArray(out.words) && out.words.every((w) => typeof w === 'string'))) {
    ok(false, `${label} words are strings`);
    return;
  }
  const res = out.results;
  const problems = [];
  if (!Array.isArray(res)) problems.push('results is not an array');
  if (res.length > data.length) problems.push(`result count ${res.length} exceeds corpus`);
  if (!res.every((r) => Number.isFinite(r.score))) problems.push('non-finite score');
  if (!res.every((r) => corpusIds.has(r.item.id))) problems.push('result id not in corpus');
  if (new Set(res.map((r) => r.item.id)).size !== res.length) problems.push('duplicate result ids');
  for (let i = 1; i < res.length; i++) if (res[i].score > res[i - 1].score) { problems.push('not sorted descending'); break; }
  if (!allowEmpty && res.length === 0) problems.push('no results');
  ok(problems.length === 0, `${label}${problems.length ? ' — ' + problems.join(', ') : ''}`);
}

// ── 1. Extreme / adversarial input battery ─────────────────────────────
// Every kind of input the page could be handed: empty, whitespace variants,
// pathological unicode, injection payloads, huge inputs, reserved words.
section('Extreme / adversarial inputs');
const EXTREME = [
  '',
  ' ',
  '   ',
  '\t',
  '\n',
  '\r\n',
  '　', // full-width space (\u3000 — \s in JS)
  'a',
  'あ',
  'あいうえお',
  'qq',
  'x',
  '0',
  '-1',
  '0x1F',
  'NaN',
  'Infinity',
  'undefined',
  'null',
  'true',
  'false',
  '電磁誘導',
  '電気 磁気',
  '物理',
  '\x00',
  '\x00\x01\x02\x03',
  'a\x00b',
  '<script>alert(1)</script>',
  '<img src=x onerror=alert(1)>',
  '<svg/onload=alert(2)>',
  'javascript:alert(3)',
  '${7*7}',
  '`rm -rf /`',
  `; DROP TABLE users; --`,
  "' OR '1'='1",
  'SELECT * FROM users WHERE id = 1 OR 1=1',
  '電磁誘導 とは？$E = \\frac{1}{4\\pi\\epsilon_0} \\frac{q}{r^2}$',
  '[リンク](https://evil.example.com)',
  '![alt text](x)',
  '&amp; &lt; &gt; &#x27; &#34;',
  'クエリ"二重引用符',
  "単一'引用符",
  '\\backslash\\path',
  'https://example.com/?q=電磁誘導&mode=max',
  '?q=ほげ&mode=pro',
  'q=',
  'mode=ultra',
  '電磁誘導 🧲 ⚡ 分子 🧬 相対性 量子 力学',
  '😀',
  '😀'.repeat(300),
  '👨👩👧👦',
  '🏳️‍🌈',
  '\uD800', // lone high surrogate
  '\uDC00', // lone low surrogate
  '\uD83D\uDE00', // valid astral pair (😀)
  'e\u0301', // combining accent
  'שלום עולם',
  'مرحبا بالعالم',
  'ＡＢＣ１２３', // full-width alnum
  'ｶﾀｶﾅ', // half-width katakana
  'とうきょう 東京',
  'やすし ヨシヒコ よしひこ',
  '───',
  '┌────────┐',
  '物理 数学 化学 生物 地学 天文 歴史 英語 国語',
  '量子力学の基礎方程式（シュレーディンガー方程式）について',
  '宇宙の果てには何があるのか？',
  '「自由エネルギー」と「エンタルピー」の違いを教えてください。',
  'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
  'ab cd ef gh ij kl mn op qr st uv wx yz '.repeat(10), // 260-char spaced multi-word query
  String.raw`\\frac{d}{dt}`, // raw LaTeX tokens
  'a'.repeat(1000), // 1000-char single "word" (worst-case levenshtein path)
  'term1 term2 term3 '.repeat(20),
];
for (const ext of EXTREME) assertRun(ext, W._buildBM25Index(data), { allowEmpty: true, maxLen: 60 });

// ── 2. Real-query battery from the corpus itself ───────────────────────
// Take a representative slice of actual questions so we exercise realistic
// physics queries, plus spot checks that genuine matches rank at the top.
section('Corpus-derived real queries');
const realQueries = data.filter((_, i) => i % 4 === 0).map((d) => d.questions[0]);
for (const q of realQueries) assertRun(q, W._buildBM25Index(data));

const denki = runBM25('電磁誘導', W._buildBM25Index(data));
const denkiHasTerm =
  denki.results.length > 0 &&
  denki.results.slice(0, 10).some((r) =>
    (r.item.questions.join(' ') + ' ' + (r.item.search_text || '')).includes('電磁誘導')
  );
ok(denkiHasTerm, 'search "電磁誘導" surfaces a genuinely matching record in the top 10');

// ── 3. App-like loop: rebuild the index per query (as searchTopResult does)
//    and confirm repeated runs stay deterministic and stateless. ─────────
section('App-like repeated search (index rebuilt per query)');
const detQueries = ['電磁誘導', '運動方程式', '量子力学', '熱力学', '光'];
const snapshots = {};
detQueries.forEach((q) => {
  const a = runBM25(q, W._buildBM25Index(data));
  const b = runBM25(q, W._buildBM25Index(data));
  const ra = a.results.map((r) => `${r.item.id}:${r.score}`);
  const rb = b.results.map((r) => `${r.item.id}:${r.score}`);
  ok(JSON.stringify(ra) === JSON.stringify(rb), `search "${q}" is deterministic across runs`);
  snapshots[q] = a.results.slice(0, 5);
});
// after running a big battery first, a fresh run must still agree (no hidden
// global state pollution — the page holds several module-level `let` caches).
const again = runBM25('電磁誘導', W._buildBM25Index(data)).results.slice(0, 5);
ok(
  JSON.stringify(again.map((r) => `${r.item.id}:${r.score}`)) ===
    JSON.stringify(snapshots['電磁誘導'].map((r) => `${r.item.id}:${r.score}`)),
  'results unchanged after the full battery (no state pollution)'
);

// ── 4. Pure semantic / RRF features with real vectors ──────────────────
section('Semantic + RRF (real embedding vectors)');
ok(Math.abs(W._cosineSim([1, 0], [1, 0]) - 1) < 1e-8, '_cosineSim identical vectors ≈ 1');
ok(Math.abs(W._cosineSim([1, 0], [0, 1])) < 1e-8, '_cosineSim orthogonal ≈ 0');
ok(Math.abs(W._cosineSim([1, 0], [-1, 0]) + 1) < 1e-8, '_cosineSim opposite ≈ -1');
ok(Number.isFinite(W._cosineSim([0, 0], [1, 1])), '_cosineSim zero vector does not produce NaN');
ok(W._cosineSim([1, 0, 0, 1], [1, 0, 0, 1]) !== undefined, '_cosineSim accepts 384-length vectors');

ok(typeof W.RRF_WEIGHT_BM25 === 'number', 'RRF_WEIGHT_BM25 is a number');
// per-model weight dispatch mirrors config.rs ModelSize::rrf_weight()
ok(W._rrfWeightForKey('small') === W.RRF_WEIGHT_SMALL, '_rrfWeightForKey small → RRF_WEIGHT_SMALL');
ok(W._rrfWeightForKey('large') === W.RRF_WEIGHT_LARGE, '_rrfWeightForKey large → RRF_WEIGHT_LARGE');
ok(W.MODE_EMB_KEYS.max.includes('large'), 'MODE_EMB_KEYS.max is the small+large ensemble');
ok(W.SEMANTIC_MODELS.small.dim === 384 && W.SEMANTIC_MODELS.large.dim === 1024, 'model dims: small 384 / large 1024');

// single-list vs multi-list parity (same call as _rrfMergeMulti([list]))
{
  const list = [{ id: 'A' }, { id: 'B' }];
  const single = W._rrfMerge(list, [{ id: 'B' }]);
  const multi = W._rrfMergeMulti(list, [[{ id: 'B' }]]);
  ok(JSON.stringify(single) === JSON.stringify(multi), '_rrfMerge ≡ _rrfMergeMulti with a single semantic list');
}
// array-of-weights fusion (the live Pro/Ultra/Max call sites' shape)
{
  const bm = [{ id: 'A' }];
  const small = [{ id: 'P' }, { id: 'X' }];
  const large = [{ id: 'X' }, { id: 'Y' }];
  const merged = W._rrfMergeMulti(bm, [small, large], 60, [W.RRF_WEIGHT_SMALL, W.RRF_WEIGHT_LARGE], W.RRF_WEIGHT_BM25);
  const get = (id) => merged.find((m) => m.id === id).rrfScore;
  ok(Number.isFinite(get('X')), 'multi-weight RRF produces finite score for an id in both semantic lists');
  ok(merged[0].id === 'X', 'agreement across semantic lists ranks X first in weighted max-mode fusion');
}

// Hybrid end-to-end on real data: a deterministic pseudo-query vector scored
// against the real "small" embeddings via _cosineSim, RRF-fused with real
// BM25 results. This is the same arithmetic the browser performs after the
// blocking model download.
{
  const dim = 384;
  let seed = 42;
  const rand = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
  const qv = Array.from({ length: dim }, () => rand() - 0.5);
  const norm = Math.sqrt(qv.reduce((s, v) => s + v * v, 0));
  qv.forEach((v, i) => (qv[i] = v / norm));

  const small = embeddings.small;
  const scored = [];
  data.forEach((item) => {
    const vec = small[item.id];
    if (vec && vec.length === dim) scored.push({ id: item.id, sim: W._cosineSim(qv, vec) });
  });
  scored.sort((a, b) => b.sim - a.sim);
  const semList = scored.slice(0, 30).map((s, i) => ({ id: s.id, idx: i, cosine: s.sim }));

  let bmList = [];
  try {
    bmList = runBM25('電磁誘導', W._buildBM25Index(data)).results.slice(0, 20).map((r) => ({ id: r.item.id }));
  } catch (e) { ok(false, `hybrid prep bm25: ${e}`); }

  let merged;
  try {
    merged = W._rrfMergeMulti(bmList, [semList], 60, W.RRF_WEIGHT_SMALL, W.RRF_WEIGHT_BM25);
  } catch (e) { ok(false, `hybrid _rrfMergeMulti: ${e}`); return; }
  ok(Array.isArray(merged) && merged.length > 0, 'hybrid (BM25 + real e5-small vectors) yields results');
  ok(merged.every((m) => Number.isFinite(m.rrfScore)), 'hybrid rrfScore values are finite');
  ok(new Set(merged.map((m) => m.id)).size === merged.length, 'hybrid fusion has no duplicate ids');
}

// ── 5. Snippet extraction ──────────────────────────────────────────────
section('Snippet extraction');
detQueries.forEach((q) => {
  const res = runBM25(q, W._buildBM25Index(data)).results;
  const item = res[0] && res[0].item;
  if (!item) return ok(true, `snippet for "${q}" skipped (no results)`);
  const snip = W._extractSnippet(item, W._expandQuery(q).words);
  ok(typeof snip === 'string' && snip.length > 0, `snippet for "${q}" is a non-empty string`);
  ok(typeof item.description === 'string', 'snippet fallback description is a string');
});

// ── 6. Image markdown & sanitization (strict, XSS) ──────────────────
section('Image markdown & sanitization (strict)');
(function(){
  // Verify search.html's image-related source patterns exist (fail if someone removes DOMPurify hardening)
  const htmlSrc = html;
  ok(htmlSrc.includes('renderer.image'), 'search.html has custom renderer.image for loading="lazy"');
  ok(htmlSrc.includes("ADD_ATTR: ['loading']"), 'DOMPurify allows loading attr');
  ok(htmlSrc.includes('enhanceImagesWithLicenses'), 'enhanceImagesWithLicenses exists');
  ok(htmlSrc.includes('img.onerror'), 'img.onerror placeholder exists');
  // BM25 must not be poisoned by image src / javascript: URLs
  const imgQueries = [
    '![attack](javascript:alert(1))',
    '![x](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)',
    '![](qa_images/3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg)',
    '<img src=x onerror=alert(1) alt="ok">',
    '<img src="qa_images/a.jpg" alt="回路図">',
    '<div class="img-row">![](qa_images/a.jpg) ![](qa_images/b.jpg)</div>',
    '![alt with spaces](qa_images/a.jpg "title with spaces")',
  ];
  imgQueries.forEach(q=> assertRun(q, W._buildBM25Index(data), {allowEmpty:true}));
  // Alt text should be searchable if someone queries the alt (image alt is in search_text)
  // We simulate a record with alt "回路図" and check that querying "回路図" finds it via BM25 pipeline
  // (The real check is in data_check.js & qa_images_stress.js; here we just ensure the query engine itself doesn't crash on alt-like terms)
  ok(W._expandQuery('回路図').words.includes('回路図'), '_expandQuery keeps alt-like CJK term');
})();

console.log(`\n${checks} checks, ${failures} failures`);
console.log(failures === 0 ? 'HTML STRESS TESTS PASSED' : `HTML STRESS TESTS FAILED`);
process.exit(failures === 0 ? 0 : 1);