#!/usr/bin/env node
// Data-artifact integrity checks for the repo's generated JSON files.
//
// Verifies (without a browser or Rust toolchain):
//   1. All JSON artifacts parse.
//   2. q_and_a_data.json record schema & referential integrity (unique ids,
//      `related` only references existing ids, required fields, types,
//      non-finite priority).
//   3. embeddings.json shape (`small`=384-d, `large`=1024-d), finite values,
//      key coverage vs the corpus.
//   4. version.json hash/size manifest matches the real files (sha256).
//   5. search_text is current: re-running the REAL pipeline
//      (`node scripts/build.js --data <copy>`) on a working copy must
//      reproduce the committed search_text byte-for-byte. Requires the
//      repo-root npm deps (kuromoji) — if they are absent this subcheck is a
//      WARN, not a failure (CI always installs them first).
//
// Usage: node scripts/tests/data_check.js
'use strict';

const fs = require('fs');
const path = require('path');
const os = require('os');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const { REPO_ROOT } = require('./_extract');

const FAIL_SOFT_MSG =
  'data_check: search_text 再生成チェックはスキップ（repo-root の node_modules が無い。npm ci を先に実行）';

let failures = 0;
let checks = 0;
let warns = 0;
function ok(cond, msg, warnOnly = false) {
  checks++;
  if (!cond) { if (warnOnly) { warns++; console.log(`WARN ${msg}`); } else { failures++; console.log(`FAIL ${msg}`); } }
}
function section(title) { console.log(`── ${title}`); }
function sha256File(p) {
  return crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
}

// ── 1. Every artifact parses ─────────────────────────────────────────
section('JSON artifacts parse');
const ARTIFACTS = ['q_and_a_data.json', 'embeddings.json', 'version.json', 'q_and_a_data_handcrafted.json'];
const parsed = {};
for (const name of ARTIFACTS) {
  try {
    parsed[name] = JSON.parse(fs.readFileSync(path.join(REPO_ROOT, name), 'utf-8'));
    ok(true, `${name} parses as JSON`);
  } catch (e) {
    ok(false, `${name} parses as JSON — ${e.message}`);
  }
}
const data = parsed['q_and_a_data.json'];
const emb = parsed['embeddings.json'];
const ver = parsed['version.json'];
const hand = parsed['q_and_a_data_handcrafted.json'];

// ── 2. Corpus schema ─────────────────────────────────────────────────
section('q_and_a_data.json schema');
ok(Array.isArray(data) && data.length > 0, `corpus is a non-empty array (${Array.isArray(data) ? data.length : 'n/a'})`);
const REQUIRED = ['id', 'questions', 'answer', 'description', 'keywords', 'synonyms', 'category', 'difficulty', 'priority', 'related', 'updated_at', 'search_text'];
data.forEach((r, i) => {
  const missing = REQUIRED.filter((k) => !(k in r));
  if (missing.length) ok(false, `record ${i} (${r.id}) missing ${missing.join(', ')}`);
  if (r.id == null || typeof r.id !== 'string' || !r.id) ok(false, `record ${i} has no usable id`);
  if (!Array.isArray(r.questions) || !r.questions.length || typeof r.questions[0] !== 'string' || !r.questions[0])
    ok(false, `record ${i} (${r.id}) needs a non-empty questions[0] (embed source)`);
  if (typeof r.answer !== 'string' || typeof r.description !== 'string')
    ok(false, `record ${i} (${r.id}) answer/description must be strings`);
  if (typeof r.priority !== 'number' || !Number.isFinite(r.priority) || r.priority < 0)
    ok(false, `record ${i} (${r.id}) priority invalid`);
  for (const arrK of ['keywords', 'synonyms', 'category', 'related']) {
    if (!Array.isArray(r[arrK]) || r[arrK].some((x) => typeof x !== 'string'))
      ok(false, `record ${i} (${r.id}) ${arrK} must be an array of strings`);
  }
  if (typeof r.search_text !== 'string' || !r.search_text.split(/\s+/).filter(Boolean).length)
    ok(false, `record ${i} (${r.id}) search_text empty`);
});
const ids = new Set(data.map((r) => r.id));
ok(ids.size === data.length, 'corpus ids are unique');
const danglingRel = data.flatMap((r) => (r.related || []).filter((id) => !ids.has(id)).map((id) => `${r.id}→${id}`));
ok(danglingRel.length === 0, `related[] references only existing ids${danglingRel.length ? ' (' + danglingRel.slice(0, 5).join(', ') + ')' : ''}`);

// ── 3. Embeddings ────────────────────────────────────────────────────
section('embeddings.json shape');
ok(emb && typeof emb === 'object', 'embeddings.json is an object');
if (emb) {
  const dims = { small: 384, large: 1024 };
  for (const key of Object.keys(dims)) {
    const set = emb[key];
    ok(set && typeof set === 'object' && !Array.isArray(set), `embeddings["${key}"] is an object map`);
    if (!set) continue;
    const vals = Object.values(set);
    ok(vals.length === data.length, `embeddings["${key}"] covers the corpus (${vals.length}/${data.length})`);
    ok(vals.every((v) => Array.isArray(v) && v.length === dims[key]), `embeddings["${key}"] vectors are ${dims[key]}-d`);
    ok(vals.every((v) => v.every((n) => Number.isFinite(n))), `embeddings["${key}"] vectors are all finite`);
    const embIds = Object.keys(set);
    ok(embIds.every((id) => ids.has(id)), `embeddings["${key}"] keys all exist in the corpus`);
    ok(ids.size - new Set(embIds).size === 0, `embeddings["${key}"] missing no corpus ids`);
    ok(new Set(embIds).size === embIds.length, `embeddings["${key}"] keys are unique`);
  }
  const topKeys = Object.keys(emb).sort();
  ok(JSON.stringify(topKeys) === JSON.stringify(['large', 'small']), `embeddings.json top-level keys are exactly small+large (got ${topKeys.join(',')})`);
}

// ── 4. version.json manifest ─────────────────────────────────────────
section('version.json manifest');
ok(ver && typeof ver === 'object' && ver.schema_version >= 1, 'version.json valid with schema_version');
if (ver && ver.files) {
  for (const f of ['q_and_a_data.json', 'embeddings.json']) {
    const meta = ver.files[f];
    if (!meta || typeof meta.hash !== 'string' || typeof meta.size !== 'number') {
      ok(false, `version.json files.${f} is malformed`);
      continue;
    }
    const real = path.join(REPO_ROOT, f);
    ok(meta.hash === sha256File(real), `version.json hash matches ${f}`);
    ok(meta.size === fs.statSync(real).size, `version.json size matches ${f}`);
  }
  ok(typeof ver.tokenizer === 'string' && ver.tokenizer, 'version.json tokenizer is set');
  ok(ver.embedding_model && ver.embedding_model.small && ver.embedding_model.large, 'version.json embedding_model.small/large set');
}

// ── 5. Handcrafted draft parses & is consistent with itself ──────────
section('q_and_a_data_handcrafted.json');
ok(Array.isArray(hand) && hand.length > 0, 'handcrafted draft is a non-empty array');
if (Array.isArray(hand)) {
  const hIds = new Set(hand.map((r) => r.id));
  ok(hIds.size === hand.length, 'handcrafted ids unique');
  hand.forEach((r, i) => {
    if (!r || typeof r.id !== 'string' || !r.id) ok(false, `handcrafted[${i}] missing id`);
    if (!Array.isArray(r.questions) || !r.questions.length || !r.questions[0]) ok(false, `handcrafted[${i}] needs questions[0]`);
    if (typeof r.answer !== 'string' || typeof r.description !== 'string') ok(false, `handcrafted[${i}] fields`);
  });
}

// ── 6. search_text is current (re-run the real pipeline) ─────────────
section('search_text regenerates deterministically');
const hasDeps = fs.existsSync(path.join(REPO_ROOT, 'node_modules', 'kuromoji'));
if (!hasDeps) {
  ok(false, FAIL_SOFT_MSG, true);
} else {
  const tmp = path.join(os.tmpdir(), `qa-determinism-${process.pid}-${Date.now()}.json`);
  fs.copyFileSync(path.join(REPO_ROOT, 'q_and_a_data.json'), tmp);
  const run = spawnSync(process.execPath, [path.join(REPO_ROOT, 'scripts', 'build.js'), '--data', tmp], {
    cwd: REPO_ROOT,
    encoding: 'utf-8',
    timeout: 120000,
  });
  if (run.status !== 0) {
    ok(false, `build.js --data exited ${run.status}: ${(run.stderr || '').slice(0, 400)}`);
  } else {
    const regen = JSON.parse(fs.readFileSync(tmp, 'utf-8'));
    let diffs = 0;
    const mine = new Map(data.map((r) => [r.id, r]));
    let orderMismatch = false;
    for (const r of regen) {
      const orig = mine.get(r.id);
      if (!orig) { orderMismatch = true; continue; }
      if (orig.search_text !== r.search_text) diffs++;
    }
    ok(diffs === 0, `committed search_text matches a fresh run (${diffs} diffs)`);
    ok(regen.length === data.length, 'regenerated corpus has the same record count');
    ok(
      JSON.stringify(regen.map((r) => r.id)) === JSON.stringify(data.map((r) => r.id)),
      'regenerated corpus preserves record order'
    );
  }
  try { fs.unlinkSync(tmp); } catch (_) { /* ignore */ }
}

console.log(`\n${checks} checks, ${warns} warns, ${failures} failures`);
console.log(failures === 0 ? 'DATA CHECKS PASSED' : 'DATA CHECKS FAILED');
process.exit(failures === 0 ? 0 : 1);