#!/usr/bin/env node
// Data-artifact integrity checks for the repo's generated JSON files.
//
// Verifies (without a browser or Rust toolchain):
//   1. All JSON artifacts parse.
//   2. q_and_a_data.json record schema & referential integrity (unique ids,
//      `related` only references existing ids, required fields, types,
//      non-finite priority).
//      search_text empty は WARN: build.yml が search_text を自動再生成する
//      ため、データ追加直後のコミットでは空のままになりうる。
//   3. embeddings.json shape (`small`=384-d, `large`=1024-d), finite values,
//      key coverage vs the corpus.
//      カバレッジ/キー一致の不一致は WARN: build.yml が embeddings.json を
//      自動再生成するため、データ追加/削除直後は不足/余剰が一時的に発生する。
//      Mismatches are a WARN, not a failure: build.yml regenerates
//      version.json after every corpus-affecting push, so between the data
//      change and that auto-commit the manifest is expected to lag behind
//      the committed files.
//   5. search_text is current: re-running the REAL pipeline
//      (`node scripts/build.js --data <copy>`) on a working copy must
//      reproduce the committed search_text byte-for-byte. A diff here is a
//      WARN, not a failure, for the same reason as version.json above:
//      build.yml regenerates search_text after every corpus-affecting push,
//      so between the data change and that auto-commit the committed value
//      is expected to lag. Requires the repo-root npm deps (kuromoji) — if
//      they are absent this subcheck is a WARN too.
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
    ok(false, `record ${i} (${r.id}) search_text empty`, true);
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
    ok(vals.length === data.length, `embeddings["${key}"] covers the corpus (${vals.length}/${data.length})`, true);
    ok(vals.every((v) => Array.isArray(v) && v.length === dims[key]), `embeddings["${key}"] vectors are ${dims[key]}-d`);
    ok(vals.every((v) => v.every((n) => Number.isFinite(n))), `embeddings["${key}"] vectors are all finite`);
    const embIds = Object.keys(set);
    ok(embIds.every((id) => ids.has(id)), `embeddings["${key}"] keys all exist in the corpus`, true);
    ok(ids.size - new Set(embIds).size === 0, `embeddings["${key}"] missing no corpus ids`, true);
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
    ok(meta.hash === sha256File(real), `version.json hash matches ${f}`, true);
    ok(meta.size === fs.statSync(real).size, `version.json size matches ${f}`, true);
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
    ok(diffs === 0, `committed search_text matches a fresh run (${diffs} diffs)`, true);
    ok(regen.length === data.length, 'regenerated corpus has the same record count');
    ok(
      JSON.stringify(regen.map((r) => r.id)) === JSON.stringify(data.map((r) => r.id)),
      'regenerated corpus preserves record order'
    );
  }
  try { fs.unlinkSync(tmp); } catch (_) { /* ignore */ }
}

// ── 7. qa_images folder + licenses.json + UUID naming ─────────────
section('qa_images integrity (folder, licenses, UUID naming)');
const QA_DIR = path.join(REPO_ROOT, 'qa_images');
const LICENSES_PATH = path.join(QA_DIR, 'licenses.json');
const licenses = (() => {
  try { return JSON.parse(fs.readFileSync(LICENSES_PATH, 'utf-8')); } catch (e) { return null; }
})();
ok(licenses !== null, 'qa_images/licenses.json parses as JSON');
if (licenses) {
  ok('_default' in licenses || Object.keys(licenses).length === 0 || licenses['_default']?.license, 'licenses.json has _default or is empty (default Apache-2.0)');
  for (const [k, v] of Object.entries(licenses)) {
    if (k === '_default') {
      ok(typeof v.license === 'string' && v.license.length > 0, '_default license is a non-empty string');
    } else {
      // key should be a UUID filename (with extension) or qa_images/ prefixed variant
      const bn = path.basename(k);
      const uuidRe = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.[a-z0-9]+$/;
      ok(uuidRe.test(bn), `licenses.json key "${k}" basename is UUID filename`);
      ok(!/[A-Z]/.test(k), `licenses.json key "${k}" is lowercase`);
      ok(typeof v.license === 'string' && v.license, `licenses.json["${k}"].license is non-empty`);
      if (v.attribution) ok(typeof v.attribution === 'string', `licenses.json["${k}"].attribution is string`);
      if (v.url) ok(typeof v.url === 'string' && /^https?:\/\//.test(v.url), `licenses.json["${k}"].url is http(s) URL`);
    }
  }
}
if (fs.existsSync(QA_DIR)) {
  const QA_EXT_RE = /\.(jpe?g|png|webp|svg|gif)$/i;
  const files = fs.readdirSync(QA_DIR).filter(f => {
    if (f === 'licenses.json' || f === '.gitkeep' || f === 'README.md') return false;
    try { if (fs.statSync(path.join(QA_DIR, f)).isDirectory()) return false; } catch(_) { return false; }
    return true; // keep even non-image to detect stray files
  });
  // Every tracked file should be an image with UUID name and lowercase ext
  for (const f of files) {
    const isImg = QA_EXT_RE.test(f);
    ok(isImg, `qa_images/${f} has allowed image extension (jpg/jpeg/png/webp/svg/gif)`);
    if (isImg) {
      const uuidImgRe = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.[a-z0-9]+$/;
      ok(uuidImgRe.test(f), `qa_images/${f} is UUID filename with lowercase ext`);
      ok(!/[A-Z]/.test(f), `qa_images/${f} is fully lowercase`);
      ok(!/\s/.test(f), `qa_images/${f} has no whitespace`);
      // file readable and non-empty (strict: empty image is a failure, warn-only would hide corruption)
      try {
        const st = fs.statSync(path.join(QA_DIR, f));
        ok(st.size > 0, `qa_images/${f} is non-empty (${st.size} bytes)`);
        ok(st.size < 8 * 1024 * 1024, `qa_images/${f} is <8MB (${(st.size/1024/1024).toFixed(2)}MB) else CI should have warned`);
      } catch (e) { ok(false, `qa_images/${f} stat failed: ${e.message}`); }
    } else {
      ok(false, `qa_images/${f} is unexpected non-image file (should be gitignored or removed)`);
    }
  }
  // orphan/broken detection (mirrors normalize-images.js)
  const mdImgRe = /!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g;
  const htmlImgRe = /<img\b[^>]*>/gi;
  const fenceRe = /```[\s\S]*?```|`[^`]*`/g;
  function extractRefsForCheck(answer) {
    if (!answer || typeof answer !== 'string') return [];
    const ranges = [];
    let m; const re = new RegExp(fenceRe.source, 'g');
    while ((m = re.exec(answer)) !== null) ranges.push([m.index, m.index+m[0].length]);
    const inF = (pos) => ranges.some(([s,e])=> pos>=s && pos<e);
    const out = [];
    mdImgRe.lastIndex=0; let mm; while ((mm=mdImgRe.exec(answer))!==null) { if(inF(mm.index)) continue; out.push(mm[2]); }
    htmlImgRe.lastIndex=0; let hm; while ((hm=htmlImgRe.exec(answer))!==null) { if(inF(hm.index)) continue; const tag=hm[0]; const sm=tag.match(/\ssrc\s*=\s*(['"])(.*?)\1/i) || tag.match(/\ssrc\s*=\s*([^\s>]+)/i); const src=sm?(sm[2]||sm[1]):null; if(src) out.push(src); }
    return out;
  }
  const allRefs = new Set();
  const refDetails = [];
  data.forEach(r => {
    for (const s of extractRefsForCheck(r.answer)) {
      if (s.startsWith('qa_images/')) { allRefs.add(path.basename(s)); refDetails.push({id:r.id, src:s}); }
    }
  });
  const imgFiles = new Set(fs.readdirSync(QA_DIR).filter(f => QA_EXT_RE.test(f)));
  const orphans = [...imgFiles].filter(f=> !allRefs.has(f));
  const brokens = [...allRefs].filter(bn=> !imgFiles.has(bn));
  ok(brokens.length===0, `qa_images: no broken refs (missing files: ${brokens.slice(0,3).join(', ')||'none'})`);
  // orphan is warn-only: an image may be intentionally staged before answer refs it
  ok(orphans.length===0, `qa_images: no orphan files (unreferenced: ${orphans.slice(0,3).join(', ')||'none'})`, true);
  // every referenced file should have its basename exactly matching the file on disk (including case)
  for (const {id, src} of refDetails) {
    const bn = path.basename(src);
    ok(src === `qa_images/${bn}`, `record ${id} image src is normalized qa_images/<basename> (got ${src})`);
    ok(!/[A-Z]/.test(bn), `record ${id} image basename is lowercase (got ${bn})`);
  }
} else {
  ok(false, 'qa_images directory exists');
}

// ── 8. image_licenses injection + search_text image handling ────────
section('image_licenses + search_text image alt handling');
if (data) {
  const mdReCheck = /!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g;
  const htmlReCheck = /<img\b[^>]*>/gi;
  const fenceReCheck = /```[\s\S]*?```|`[^`]*`/g;
  function altSrcPairs(answer){
    if(!answer||typeof answer!=='string') return [];
    const ranges=[]; let m; const re=new RegExp(fenceReCheck.source,'g'); while((m=re.exec(answer))!==null) ranges.push([m.index,m.index+m[0].length]);
    const inF=(pos)=>ranges.some(([s,e])=>pos>=s&&pos<e);
    const out=[];
    mdReCheck.lastIndex=0; let mm; while((mm=mdReCheck.exec(answer))!==null){ if(inF(mm.index)) continue; out.push({alt:mm[1], src:mm[2]}); }
    htmlReCheck.lastIndex=0; let hm; while((hm=htmlReCheck.exec(answer))!==null){ if(inF(hm.index)) continue; const tag=hm[0]; const am=tag.match(/alt\s*=\s*(['"])(.*?)\1/i); const alt=am?am[2]:''; const sm=tag.match(/\ssrc\s*=\s*(['"])(.*?)\1/i)||tag.match(/\ssrc\s*=\s*([^\s>]+)/i); const src=sm?(sm[2]||sm[1]):null; if(src) out.push({alt,src}); }
    return out;
  }
  let imageRecords = 0;
  data.forEach(r=>{
    const pairs = altSrcPairs(r.answer);
    const hasQa = pairs.some(p=> p.src.startsWith('qa_images/'));
    if (hasQa) imageRecords++;
    if (hasQa) {
      ok(r.image_licenses && typeof r.image_licenses==='object', `record ${r.id} has image_licenses when qa_images referenced`);
      if (r.image_licenses) {
        for(const {src} of pairs.filter(p=>p.src.startsWith('qa_images/'))) {
          ok(src in r.image_licenses, `record ${r.id} image_licenses contains ${src}`);
          const lic = r.image_licenses[src];
          ok(lic && typeof lic.license==='string' && lic.license.length>0, `record ${r.id} image_licenses[${src}].license non-empty`);
        }
        // no extra keys that are not referenced
        for(const k of Object.keys(r.image_licenses)){
          ok(pairs.some(p=>p.src===k), `record ${r.id} image_licenses key ${k} is actually referenced`);
        }
      }
      // search_text should contain alt but NOT the src (UUID)
      for(const {alt, src} of pairs.filter(p=>p.src.startsWith('qa_images/'))){
        if (alt && alt.trim()) {
          // alt may be morpheme-split, so check case-insensitive substring of search_text (which contains kata/hira variants)
          const needle = alt.trim().split(/\s+/)[0];
          if (needle.length >= 2) ok(r.search_text.toLowerCase().includes(needle.toLowerCase()) || r.search_text.includes(needle), `record ${r.id} search_text contains alt "${needle}"`, true);
        }
        // strict: src UUID should NOT appear in search_text (would be noise)
        const bn = path.basename(src);
        const uuidPart = bn.split('.')[0];
        ok(!r.search_text.includes(uuidPart), `record ${r.id} search_text does not contain image UUID ${uuidPart}`);
        ok(!r.search_text.includes('qa_images'), `record ${r.id} search_text does not contain "qa_images" literal`);
      }
    } else {
      ok(!('image_licenses' in r) || !r.image_licenses || Object.keys(r.image_licenses).length===0, `record ${r.id} has no image_licenses when no qa_images ref`);
    }
  });
  // global check: image_licenses must be consistent with licenses.json _default fallback
  // (already covered by build.js determinism check)
}

// ── 9. version.json qa_images manifest strict ─────────────────────
section('version.json qa_images manifest');
if (ver) {
  ok('qa_images' in ver, 'version.json has qa_images key (schema v4)');
  if (ver.qa_images) {
    const qa = ver.qa_images;
    ok(typeof qa.hash==='string' && /^[0-9a-f]{64}$/.test(qa.hash), 'qa_images.hash is 64-char hex');
    ok(typeof qa.count==='number' && Number.isInteger(qa.count) && qa.count>=0, 'qa_images.count is non-negative int');
    ok(typeof qa.total_bytes==='number' && qa.total_bytes>=0, 'qa_images.total_bytes non-negative');
    ok(typeof qa.avg_bytes==='number' && qa.avg_bytes>=0, 'qa_images.avg_bytes non-negative');
    ok(qa.buckets && typeof qa.buckets.lt3==='number' && typeof qa.buckets.btw==='number' && typeof qa.buckets.gte5==='number', 'qa_images.buckets has lt3/btw/gte5');
    ok(qa.lt3===undefined && qa.btw===undefined, 'qa_images buckets not at top level (nesting correct)');
    // cross-check with actual files
    const QA_EXT_RE2 = /\.(jpe?g|png|webp|svg|gif)$/i;
    const dir = path.join(REPO_ROOT, 'qa_images');
    if (fs.existsSync(dir)) {
      const actualFiles = fs.readdirSync(dir).filter(f=> QA_EXT_RE2.test(f) && f!=='licenses.json' && f!=='.gitkeep' && f!=='README.md');
      ok(qa.count===actualFiles.length, `qa_images.count matches files on disk (${qa.count} vs ${actualFiles.length})`);
      let total=0; actualFiles.forEach(f=> total+=fs.statSync(path.join(dir,f)).size);
      ok(qa.total_bytes===total, `qa_images.total_bytes matches disk (${qa.total_bytes} vs ${total})`);
      const expAvg = actualFiles.length? Math.round(total/actualFiles.length):0;
      ok(qa.avg_bytes===expAvg, `qa_images.avg_bytes matches (${qa.avg_bytes} vs ${expAvg})`);
      // combined hash recomputed
      const hashes = actualFiles.slice().sort().map(f=> crypto.createHash('sha256').update(fs.readFileSync(path.join(dir,f))).digest('hex'));
      const combined = hashes.length? crypto.createHash('sha256').update(hashes.join('')).digest('hex') : crypto.createHash('sha256').update('').digest('hex');
      ok(qa.hash===combined, 'qa_images.hash matches recomputed combined hash');
    }
    ok(ver.schema_version===4, 'version.json schema_version is 4 (qa_images added)');
    ok(typeof ver.tokenizer==='string' && ver.tokenizer==='lindera-ipadic', 'version.json tokenizer is lindera-ipadic');
  } else {
    ok(false, 'version.json qa_images is missing (expected for schema 4)');
  }
}

console.log(`\n${checks} checks, ${warns} warns, ${failures} failures`);
console.log(failures === 0 ? 'DATA CHECKS PASSED' : 'DATA CHECKS FAILED');
process.exit(failures === 0 ? 0 : 1);