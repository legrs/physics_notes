#!/usr/bin/env node
// Generates the physq stress-test case file `physq_stress_cases.jsonl`.
//
// One {"query": "…"} per line, consumed by:
//   physq --model none --data q_and_a_data.json eval --cases physq_stress_cases.jsonl
// (`--model none` = BM25-only, so no ~470MB model download in CI.) Batched
// into a single eval process; each field is a smoke/stress probe that also
// exercises the CLI's JSON-level handling of extreme inputs.
//
// Usage: node scripts/tests/gen_stress_cases.js   (writes the .jsonl in place)
'use strict';

const fs = require('fs');
const path = require('path');

const queries = [
  // realistic physics queries
  '電磁誘導',
  '電気 磁気',
  '運動方程式',
  '量子力学 シュレーディンガー',
  '熱力学 第一法則',
  '光の屈折 反射',
  '交流 回路 インピーダンス',
  '力積 運動量保存',
  '万有引力 ケプラーの法則',
  '電力 P=VI',
  '波の干渉',
  '原子核 放射性崩壊',
  'relative velocity',
  'conservation of energy',
  'Newton second law',
  // kana variants (corpus search_text carries both)
  'でんじゆうどう',
  'ブツリ ガクシュウ',
  'うんどうほうていしき',
  // extreme / adversarial inputs
  '',
  ' ',
  '　',
  'aaaa',
  'a'.repeat(300),
  'x'.repeat(5000),
  '\x00\x01\x02',
  '<script>alert(1)</script>',
  '${7*7}',
  '`rm -rf /`',
  "'; DROP TABLE users; --",
  "' OR '1'='1",
  'SELECT * FROM users;',
  'NaN',
  'Infinity',
  'undefined',
  'null',
  'true/false',
  '-1',
  '0x1F',
  '😀🧲⚡',
  '👨👩👧👦',
  '🏳️‍🌈',
  '\uD800',
  '\uDC00',
  'ｅ\u0301',
  'שלום',
  'مرحبا',
  'ＡＢＣ１２３',
  'ｶﾀｶﾅ',
  '───',
  '┌─┐',
  '物理 数学 化学 生物 地学 天文',
  '量子力学とは何か 具体例も交えて教えて ください 長文',
  '宇宙 の 果て に は 何 が ある のか',
  'https://example.com/?q=電磁誘導&mode=max',
  '?q=ほげ&mode=ultra',
  'q=',
  'mode=fast',
  '＜html＞タグ',
  '&amp;&lt;&gt;',
  '半角ｶﾅ 全角カナ ひらがな',
  'aaa bbb ccc ddd eee fff ggg hhh iii jjj',
];

// `physq eval` needs a `target` (the corpus record id that *should* win). For
// each query pick the first record whose search_text contains it (a plausible
// target for a realistic query); fall back to the first record otherwise —
// evaluate still reports null rank when the target isn't in the results,
// which is exactly the edge-input behavior we want to probe.
const corpus = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'q_and_a_data.json'), 'utf-8'));
function pickTarget(q) {
  const norm = q.toLowerCase();
  const hit = corpus.find((r) => (r.search_text || '').toLowerCase().includes(norm));
  return hit ? hit.id : corpus[0].id;
}

const lines = queries.map((query) =>
  JSON.stringify({ query, target: pickTarget(query) })
);
const out = lines.join('\n') + '\n';
const target = path.join(__dirname, 'physq_stress_cases.jsonl');
fs.writeFileSync(target, out);
console.log(`wrote ${lines.length} cases to ${target}`);