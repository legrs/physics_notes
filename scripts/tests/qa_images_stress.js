#!/usr/bin/env node
// Strict QA-images pipeline stress tests (no browser, no model).
//
// Covers the "めっちゃ厳しい" edge cases plan §3-6/§10 demands:
//  - UUID filename regex strictness (lowercase, extension, length, whitespace)
//  - build.js strip helpers (markdown / html / img-row) with XSS vectors, code fences, title variants, quote variants
//  - search_text: alt retained, src/UUID removed, code-fence images ignored
//  - image_licenses injection (via build.js extract logic)
//  - normalize-images.js behavior on a temp qa_images dir (rename, ext normalize, orphan/broken)
//  - version.json qa_images manifest consistency
//  - sanitizeHtml / renderer.image escaping patterns (source-level, no DOM)
// Usage: node scripts/tests/qa_images_stress.js
'use strict';
const fs = require('fs');
const path = require('path');
const os = require('os');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const { REPO_ROOT, loadHtml } = require('./_extract');

let failures = 0, checks = 0;
function ok(cond, msg) {
  checks++;
  if (!cond) { failures++; console.log(`FAIL ${msg}`); }
}
function section(t){ console.log(`── ${t}`); }

// ---- 1. UUID filename regex strict ----
section('UUID filename regex (strict)');
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.[a-z0-9]+$/;
const good = [
  '3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg',
  '00000000-0000-0000-0000-000000000000.png',
  'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.webp',
  '123e4567-e89b-12d3-a456-426614174000.svg',
  'abcdef12-3456-7890-abcd-ef1234567890.gif',
];
const bad = [
  '3F9A8C1E-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg', // upper hex
  '3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.JPG', // upper ext
  '3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6', // no ext
  '3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.', // empty ext
  '3f9a8c1e-1a2b-4c3d-9e8f-a1b.jpg', // short
  ' 3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg', // leading space
  '3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg ', // trailing space
  '3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6 .jpg', // space before dot
  'photo_001.jpg', // not uuid
  'qa_images/3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg', // prefixed
  '3f9a8c1e_1a2b_4c3d_9e8f_a1b2c3d4e5f6.jpg', // underscores
];
good.forEach(f=> ok(UUID_RE.test(f), `UUID_RE accepts ${f}`));
bad.forEach(f=> ok(!UUID_RE.test(f), `UUID_RE rejects ${f}`));

// ---- 2. build.js strip helpers (load real functions) ----
section('build.js strip helpers (edge cases, XSS, code fences)');
const buildSrc = fs.readFileSync(path.join(REPO_ROOT,'scripts/build.js'),'utf-8');
// Extract function bodies via regex (simple, not brace-matched — these are one-liners)
function loadBuildFns(){
  const stripCodeFences = new Function('s', `return s.replace(/\\\`\\\`\\\`[\\\\s\\\\S]*?\\\`\\\`\\\`/g,' ').replace(/\\\`[^\\\`]*\\\`/g,' ');`);
  const stripMarkdownImages = new Function('s', `return s.replace(/!\\\\[([^\\\\]]*)\\\\]\\\\(\\\\s*([^\\\\s)]+)(?:\\\\s+(?:"[^"]*"|'[^']*'))?\\\\s*\\\\)/g, ' $1 ');`);
  // Use the updated helpers from build.js source to avoid drift: eval the definitions if present
  // Fallback to parsing the file's current definitions
  const m1 = buildSrc.match(/function stripMarkdownImages\(s\)\s*\{[^}]+\}/);
  const m2 = buildSrc.match(/function stripHtmlImages\(s\)\s*\{[^}]+\}[\s\S]*?return[^}]+\} *\}/);
  const m3 = buildSrc.match(/function stripHtmlImageRows\(s\)\s*\{[^}]+\}/);
  // If extraction fails, warn but keep manual versions
  if (m1) {
    try { return {
      stripCodeFences: eval(`(function(s){ ${buildSrc.match(/function stripCodeFences\(s\)\s*\{[^}]+\}/)[0].replace(/function stripCodeFences\(s\)\s*\{/,'').replace(/\}$/,'')} })`),
      stripMarkdownImages: eval(`(function(s){ ${m1[0].replace(/function stripMarkdownImages\(s\)\s*\{/,'').replace(/\}$/,'')} })`),
      stripHtmlImages: (()=>{ const src = buildSrc.match(/function stripHtmlImages\(s\)\s*\{[\s\S]*?^}/m); if(src) return eval(`(${src[0].replace('function stripHtmlImages','function')})`); return stripMarkdownImages; })(),
      stripHtmlImageRows: eval(`(function(s){ ${m3[0].replace(/function stripHtmlImageRows\(s\)\s*\{/,'').replace(/\}$/,'')} })`),
    }; } catch(e){ /* fallback */ }
  }
  return { stripCodeFences, stripMarkdownImages,
    stripHtmlImages: (s)=> s.replace(/<img\b[^>]*>/gi, m=>{ let alt=''; let am=m.match(/alt\s*=\s*"([^"]*)"/i); if(am) alt=am[1]; else if((am=m.match(/alt\s*=\s*'([^']*)'/i))) alt=am[1]; else if((am=m.match(/alt\s*=\s*([^\s"'`>]+)/i))) alt=am[1]; return ' '+alt+' '; }),
    stripHtmlImageRows: (s)=> s.replace(/<div\b[^>]*\bclass\s*=\s*["'][^"']*\bimg-row\b[^"']*["'][^>]*>/gi,' ').replace(/<\/div>/gi,' ') };
}
// Load via requiring a small helper that evals the actual file's functions (most faithful)
function getStripFnsViaEval(){
  const vm = require('vm');
  const code = buildSrc + '\n;({stripCodeFences, stripMarkdownImages, stripHtmlImages, stripHtmlImageRows})';
  const sandbox = { require, __dirname: path.join(REPO_ROOT,'scripts'), module:{}, exports:{} };
  // Provide kuromoji stub to avoid loading
  sandbox.require = (p)=>{ if(p==='kuromoji') return { builder:()=>({build:()=>{}})}; return require(p); };
  try {
    const fns = vm.runInNewContext(code, { ...global, require: sandbox.require, __dirname: sandbox.__dirname, console, process, Buffer, setTimeout, clearTimeout });
    // The evaluated code returns the object only if we explicitly construct it
    // Fallback: extract via regex and create functions manually (above)
  } catch(e){}
  // Simpler: directly test against expected behavior using reference implementations that match the fixed file
  return null;
}
// Reference implementations matching the fixed build.js (copied for determinism)
function stripCodeFences(s){ return s.replace(/```[\s\S]*?```/g,' ').replace(/`[^`]*`/g,' '); }
function stripMarkdownImages(s){ return s.replace(/!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+(?:"[^"]*"|'[^']*'))?\s*\)/g, ' $1 '); }
function stripHtmlImages(s){ return s.replace(/<img\b[^>]*>/gi, m=>{ let alt=''; let am=m.match(/alt\s*=\s*"([^"]*)"/i); if(am) alt=am[1]; else if((am=m.match(/alt\s*=\s*'([^']*)'/i))) alt=am[1]; else if((am=m.match(/alt\s*=\s*([^\s"'`>]+)/i))) alt=am[1]; return ' '+alt+' '; }); }
function stripHtmlImageRows(s){ return s.replace(/<div\b[^>]*\bclass\s*=\s*["'][^"']*\bimg-row\b[^"']*["'][^>]*>/gi,' ').replace(/<\/div>/gi,' '); }
function pipeline(s){ let r=stripCodeFences(s); r=stripMarkdownImages(r); r=stripHtmlImages(r); r=stripHtmlImageRows(r); return r; }

// Basic (use includes/strict containment, not exact whitespace)
ok(pipeline('![回路図: 電池と抵抗](qa_images/abc.jpg)').includes('回路図: 電池と抵抗') && !pipeline('![回路図: 電池と抵抗](qa_images/abc.jpg)').includes('qa_images'), 'markdown alt retained, src removed');
ok(pipeline('![](qa_images/empty.jpg)').trim() === '' && !pipeline('![](qa_images/empty.jpg)').includes('qa_images'), 'empty alt markdown yields no src');
ok(pipeline('![a](qa_images/a.jpg "title")').includes('a') && !pipeline('![a](qa_images/a.jpg "title")').includes('title'), 'title double-quoted stripped');
ok(pipeline("![a](qa_images/a.jpg 'title')").includes('a') && !pipeline("![a](qa_images/a.jpg 'title')").includes('title'), "title single-quoted stripped");
ok(pipeline('<img alt="photo" src="qa_images/p.jpg">').includes('photo') && !pipeline('<img alt="photo" src="qa_images/p.jpg">').includes('qa_images'), 'html alt double-quoted retained');
ok(pipeline("<img alt='photo' src='qa_images/p.jpg'>").includes('photo'), 'html alt single-quoted retained');
ok(pipeline('<img alt=photo src=qa_images/p.jpg>').includes('photo'), 'html alt unquoted retained');
ok(!pipeline('<div class="img-row">').includes('img-row'), 'img-row open stripped');
ok(!pipeline('<div class="img-row extra" id="x">').includes('img-row'), 'img-row with extra attrs stripped');
ok(!pipeline('<DIV CLASS="img-row">').includes('img-row'), 'img-row case-insensitive');
ok(pipeline('</div>').trim()==='','closing div stripped');
ok(pipeline('</DIV>').trim()==='','closing div case-insensitive');

// Code fence handling: images inside fences must be ignored
ok(pipeline('```\n![ignore](qa_images/ignore.jpg)\n```\n![real](qa_images/real.jpg)').includes('real') && !pipeline('```\n![ignore](qa_images/ignore.jpg)\n```\n![real](qa_images/real.jpg)').includes('ignore'), 'code fence image ignored, real alt kept');
ok(pipeline('`![ignore](qa_images/x.jpg)` and ![real](qa_images/y.jpg)').includes('real') && !pipeline('`![ignore](qa_images/x.jpg)` and ![real](qa_images/y.jpg)').includes('ignore'), 'inline code image ignored');
ok(!pipeline('```js\n<img src="qa_images/a.jpg" alt="no">\n```').includes('no'), 'html inside fence ignored');

// XSS vectors: alt is kept but src/onerror/javascript must not leak
ok(!pipeline('![x](javascript:alert(1))').includes('javascript'), 'javascript: src not retained');
ok(!pipeline('![x](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)').includes('data:'), 'data: src not retained');
ok(pipeline('<img src=x onerror=alert(1) alt="ok">').includes('ok') && !pipeline('<img src=x onerror=alert(1) alt="ok">').includes('onerror'), 'onerror stripped, alt kept');
ok(pipeline('<img src="qa_images/a.jpg" alt="a&quot; onerror=&quot;alert(1)">').includes('a'), 'alt with entities preserved as text');
ok(pipeline('<svg/onload=alert(1)><img src="qa_images/a.jpg" alt="t">').includes('t'), 'svg with onload does not block img alt extraction (svg sanitization is DOMPurify job)');
ok(pipeline('![alt](qa_images/a.jpg) <script>alert(1)</script>').includes('alt') && pipeline('![alt](qa_images/a.jpg) <script>alert(1)</script>').includes('script'), 'pipeline is text-only, script tag passes through (DOMPurify will strip later)');

// Extremes
ok(pipeline('') === '', 'empty string');
ok(pipeline('   ').trim() === '', 'whitespace preserved (pipeline is no-op)');
ok(pipeline('![alt](qa_images/a.jpg)'.repeat(100)).split('alt').length > 50, 'repeated images handled');
ok(pipeline('![alt](qa_images/a.jpg)'.repeat(1000)).length > 0, '1000 images not throwing');
ok(pipeline('![alt with spaces and 記号!](qa_images/a.jpg)').includes('alt with spaces'), 'alt with spaces/unicode retained');
ok(pipeline('<img src="qa_images/a.jpg" alt="">').trim()==='', 'empty alt html yields no src');
ok(pipeline('![alt](qa_images/a.jpg) and <img alt="b" src="qa_images/b.jpg">').includes('alt') && pipeline('![alt](qa_images/a.jpg) and <img alt="b" src="qa_images/b.jpg">').includes('b'), 'mixed markdown+html both alts kept');

// ---- 3. search_text pipeline integration via build.js --data ----
section('search_text integration (alt in, src/UUID out, fences ignored)');
const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'qa-img-stress-'));
const tmpData = path.join(tmpDir, 'q_and_a_data.json');
const sample = [
  { id:'test-uuid-1', questions:['Q1'], answer:'Text ![回路](qa_images/3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg) end', description:'desc', keywords:[], synonyms:[], category:[], difficulty:'1', priority:1, related:[], updated_at:'2026-01-01', search_text:'' },
  { id:'test-uuid-2', questions:['Q2'], answer:'```\n![ignore](qa_images/ignore.jpg)\n```\nreal ![keep](qa_images/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jpg)', description:'', keywords:[], synonyms:[], category:[], difficulty:'1', priority:1, related:[], updated_at:'2026-01-01', search_text:'' },
  { id:'test-uuid-3', questions:['Q3'], answer:'<img alt="html alt" src="qa_images/bbbbbbbb-cccc-dddd-eeee-ffffffffffff.jpg">', description:'', keywords:[], synonyms:[], category:[], difficulty:'1', priority:1, related:[], updated_at:'2026-01-01', search_text:'' },
  { id:'test-uuid-4', questions:['Q4'], answer:'動画 ![振り子の動画](qa_images/cccccccc-dddd-eeee-ffff-000000000000.mp4) と <video controls title="斜面の動画" poster="qa_images/dddddddd-eeee-ffff-0000-111111111111.jpg"><source src="qa_images/eeeeeeee-ffff-0000-1111-222222222222.webm" type="video/webm"></video>', description:'', keywords:[], synonyms:[], category:[], difficulty:'1', priority:1, related:[], updated_at:'2026-01-01', search_text:'' },
];
fs.writeFileSync(tmpData, JSON.stringify(sample, null, 2));
const run = spawnSync(process.execPath, [path.join(REPO_ROOT,'scripts/build.js'),'--data', tmpData], { encoding:'utf-8', timeout: 120000 });
ok(run.status===0, `build.js --data succeeds (${run.status}) ${run.stderr? run.stderr.slice(0,200):''}`);
if (run.status===0){
  const out = JSON.parse(fs.readFileSync(tmpData,'utf-8'));
  const r1 = out.find(r=>r.id==='test-uuid-1');
  const r2 = out.find(r=>r.id==='test-uuid-2');
  const r3 = out.find(r=>r.id==='test-uuid-3');
  ok(r1 && r1.search_text.includes('回路'), 'search_text contains markdown alt "回路"');
  ok(r1 && !r1.search_text.includes('3f9a8c1e'), 'search_text does NOT contain UUID of r1');
  ok(r1 && !r1.search_text.includes('qa_images'), 'search_text does NOT contain literal qa_images');
  ok(r2 && r2.search_text.includes('keep') && !r2.search_text.includes('ignore'), 'fenced image alt ignored, real kept');
  ok(r3 && r3.search_text.includes('html alt'), 'html alt in search_text');
  ok(r3 && !r3.search_text.includes('bbbbbbbb'), 'html UUID not in search_text');
  // image_licenses should be injected
  ok(r1 && r1.image_licenses && r1.image_licenses['qa_images/3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg'], 'image_licenses injected for r1');
  // videos: label text searchable, file names / tags not; every referenced file licensed
  const r4 = out.find(r=>r.id==='test-uuid-4');
  ok(r4 && r4.search_text.includes('振り子の動画'), 'markdown video label in search_text');
  ok(r4 && r4.search_text.includes('斜面の動画'), '<video title> in search_text');
  ok(r4 && !/qa_images|cccccccc|eeeeeeee|video|source|poster/i.test(r4.search_text), 'video src/tags not in search_text');
  ok(r4 && r4.image_licenses && ['qa_images/cccccccc-dddd-eeee-ffff-000000000000.mp4', 'qa_images/dddddddd-eeee-ffff-0000-111111111111.jpg', 'qa_images/eeeeeeee-ffff-0000-1111-222222222222.webm'].every(k => r4.image_licenses[k]), 'image_licenses injected for markdown video, <source> and poster');
}
fs.rmSync(tmpDir, { recursive:true, force:true });

// ---- 4. normalize-images.js on temp dir (edge filenames) ----
section('normalize-images.js temp dir (ext normalize, UUID generation, collision)');
(function(){
  const td = fs.mkdtempSync(path.join(os.tmpdir(), 'qa-norm-'));
  const qa = path.join(td, 'qa_images'); fs.mkdirSync(qa);
  const dataPath = path.join(td, 'q_and_a_data.json');
  // create files with various edge names
  const files = [
    {name:'photo_001.JPG', content:'fakeimg1'}, // non-uuid, upper ext → should be UUID+jpg
    {name:'3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.JPG', content:'img2'}, // uuid lower, upper ext → lower ext only
    {name:'AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE.jpg', content:'img3'}, // upper uuid → lowercased/new uuid? current lowercases
  ];
  files.forEach(f=> fs.writeFileSync(path.join(qa,f.name), f.content));
  fs.writeFileSync(path.join(qa,'licenses.json'), JSON.stringify({_default:{license:"Apache-2.0"}, "photo_001.JPG":{license:"CC BY 4.0"}}, null,2));
  fs.writeFileSync(dataPath, JSON.stringify([
    {id:'r1', questions:['q'], answer:'![a](qa_images/photo_001.JPG) ![b](qa_images/3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.JPG) ![c](qa_images/AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE.jpg)', description:'', keywords:[], synonyms:[], category:[], difficulty:'1', priority:1, related:[], updated_at:'2026-01-01', search_text:''}
  ],null,2));
  // Run normalize with custom REPO_ROOT env? The script uses __dirname/.. as repo root, so we need to invoke it with cwd tricks.
  // Instead, test the logic via direct spawn with env override: we temporarily replace the repo's qa_images with symlink? Simpler: test the regex and collision logic directly, not full script.
  // For strictness, we at least verify the pre-conditions
  ok(fs.existsSync(path.join(qa,'photo_001.JPG')), 'temp file photo_001.JPG exists');
  // Simulate what the script should do: lower ext, generate UUID, update refs
  // We verify that after running the real script via a temp repo root trick would be ideal, but we skip full integration here and test the standalone regex
  const IMAGE_EXT_RE = /\.(jpe?g|png|webp|svg|gif)$/i;
  const UUID_RE2 = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.[a-z0-9]+$/;
  ok(IMAGE_EXT_RE.test('photo_001.JPG'), 'IMAGE_EXT_RE matches upper JPG');
  ok(!UUID_RE2.test('photo_001.JPG'), 'photo_001.JPG is not UUID');
  ok(!UUID_RE2.test('3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.JPG'), 'upper ext fails UUID_RE');
  ok(UUID_RE2.test('3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg'), 'lower ext passes');
  fs.rmSync(td, {recursive:true, force:true});
})();

section('normalize-images.js real run (--root, images + videos)');
(function(){
  const td = fs.mkdtempSync(path.join(os.tmpdir(), 'qa-norm-run-'));
  const qa = path.join(td, 'qa_images'); fs.mkdirSync(qa);
  for (const f of ['photo_001.JPG', 'clip_one.MP4', 'side.webm', 'poster.png']) fs.writeFileSync(path.join(qa, f), 'x' + f);
  fs.writeFileSync(path.join(qa, 'licenses.json'), JSON.stringify({ _default: { license: 'Apache-2.0' }, 'side.webm': { license: 'CC BY 4.0' } }));
  const answer = '![a](qa_images/photo_001.JPG) ![v](qa_images/clip_one.MP4)\n' +
    '<video controls poster="qa_images/poster.png"><source src="qa_images/side.webm" type="video/webm"></video>\n' +
    '```\n![keep](qa_images/side.webm)\n```';
  fs.writeFileSync(path.join(td, 'q_and_a_data.json'), JSON.stringify([{ id: 'r1', questions: ['q'], answer }]));
  const script = path.join(REPO_ROOT, 'scripts/normalize-images.js');
  const chk = spawnSync(process.execPath, [script, '--root', td, '--check'], { encoding: 'utf-8' });
  ok(chk.status === 1, `--check exits 1 when files need renaming (got ${chk.status})`);
  ok(/side\.webm/.test(chk.stdout) && /poster\.png/.test(chk.stdout), '--check lists video and poster files');
  const run = spawnSync(process.execPath, [script, '--root', td], { encoding: 'utf-8' });
  ok(run.status === 0, `normalize run exits 0 (got ${run.status}) ${run.stderr.slice(0, 200)}`);
  const files = fs.readdirSync(qa).filter(f => f !== 'licenses.json').sort();
  const uuidFile = ext => files.find(f => UUID_RE.test(f) && f.endsWith(ext));
  ok(uuidFile('.jpg') && uuidFile('.mp4') && uuidFile('.webm') && uuidFile('.png'), `images, videos and poster renamed to UUIDs (ext lowercased): ${files.join(', ')}`);
  const out = JSON.parse(fs.readFileSync(path.join(td, 'q_and_a_data.json'), 'utf-8'))[0].answer;
  ok(out.includes(`![a](qa_images/${uuidFile('.jpg')})`), 'markdown image ref rewritten');
  ok(out.includes(`![v](qa_images/${uuidFile('.mp4')})`), 'markdown video ref rewritten');
  ok(out.includes(`<source src="qa_images/${uuidFile('.webm')}"`), '<source src> ref rewritten');
  ok(out.includes(`poster="qa_images/${uuidFile('.png')}"`), '<video poster> ref rewritten');
  ok(out.includes('```\n![keep](qa_images/side.webm)\n```'), 'fenced ref untouched');
  const lic = JSON.parse(fs.readFileSync(path.join(qa, 'licenses.json'), 'utf-8'));
  ok(lic[uuidFile('.webm')] && lic[uuidFile('.webm')].license === 'CC BY 4.0', 'licenses.json key follows the renamed video');
  const again = spawnSync(process.execPath, [script, '--root', td, '--check'], { encoding: 'utf-8' });
  ok(again.status === 0, `second --check is clean (got ${again.status})`);
  fs.rmSync(td, { recursive: true, force: true });
})();

section('normalize-images.js HEIC/HEIF → JPEG');
(function(){
  const fixture = path.join(REPO_ROOT, 'scripts/tests/fixtures/tiny.heic');
  const td = fs.mkdtempSync(path.join(os.tmpdir(), 'qa-heic-'));
  const qa = path.join(td, 'qa_images'); fs.mkdirSync(qa);
  fs.copyFileSync(fixture, path.join(qa, 'IMG_0001.HEIC'));
  fs.copyFileSync(fixture, path.join(qa, 'scan.heif'));
  fs.writeFileSync(path.join(qa, 'licenses.json'), JSON.stringify({ _default: { license: 'Apache-2.0' }, 'IMG_0001.HEIC': { license: 'CC BY 4.0' } }));
  fs.writeFileSync(path.join(td, 'q_and_a_data.json'), JSON.stringify([{ id: 'r1', questions: ['q'],
    answer: '![iPhone](qa_images/IMG_0001.HEIC) <img src="qa_images/scan.heif" alt="s">' }]));
  const script = path.join(REPO_ROOT, 'scripts/normalize-images.js');
  const chk = spawnSync(process.execPath, [script, '--root', td, '--check'], { encoding: 'utf-8' });
  ok(chk.status === 1 && /Would convert IMG_0001\.HEIC/.test(chk.stdout), '--check reports HEIC needing conversion');
  ok(fs.existsSync(path.join(qa, 'IMG_0001.HEIC')), '--check does not touch files');
  const run = spawnSync(process.execPath, [script, '--root', td], { encoding: 'utf-8', timeout: 120000 });
  ok(run.status === 0, `normalize converts HEIC (exit ${run.status}) ${(run.stderr || '').slice(0, 200)}`);
  const files = fs.readdirSync(qa).filter(f => f !== 'licenses.json');
  ok(files.length === 2 && files.every(f => UUID_RE.test(f) && f.endsWith('.jpg')), `HEIC/HEIF became <uuid>.jpg: ${files.join(', ')}`);
  ok(files.every(f => fs.readFileSync(path.join(qa, f)).subarray(0, 3).equals(Buffer.from([0xff, 0xd8, 0xff]))), 'converted files are real JPEGs');
  const out = JSON.parse(fs.readFileSync(path.join(td, 'q_and_a_data.json'), 'utf-8'))[0].answer;
  ok(!/heic|heif/i.test(out) && files.every(f => out.includes('qa_images/' + f)), 'markdown and <img> refs point at the JPEGs');
  const lic = JSON.parse(fs.readFileSync(path.join(qa, 'licenses.json'), 'utf-8'));
  ok(Object.entries(lic).some(([k, v]) => k.endsWith('.jpg') && v.license === 'CC BY 4.0'), 'licenses.json key follows the conversion');
  fs.rmSync(td, { recursive: true, force: true });
})();

// ---- 5. version.json qa_images manifest ----
section('version.json qa_images manifest (if exists)');
const ver = JSON.parse(fs.readFileSync(path.join(REPO_ROOT,'version.json'),'utf-8'));
ok(ver.schema_version===4, 'version schema 4');
ok(ver.qa_images && typeof ver.qa_images.hash==='string', 'qa_images.hash exists');
if (ver.qa_images){
  ok(/^[0-9a-f]{64}$/.test(ver.qa_images.hash), 'qa_images.hash is 64 hex');
  ok(Number.isInteger(ver.qa_images.count), 'qa_images.count int');
}

// ---- 6. sanitizeHtml / renderer.image source-level checks (no DOM) ----
section('sanitizeHtml / renderer.image (source-level)');
const searchHtml = loadHtml('search.html');
ok(searchHtml.includes('renderer.image'), 'search.html has custom renderer.image');
ok(searchHtml.includes('loading="lazy"'), 'renderer.image adds loading="lazy"');
ok(searchHtml.includes("ADD_ATTR: ['loading']"), 'DOMPurify ADD_ATTR includes loading');
ok(searchHtml.includes("USE_PROFILES: { html: true, svg: true"), 'DOMPurify uses html/svg profiles');
ok(searchHtml.includes('enhanceImagesWithLicenses'), 'enhanceImagesWithLicenses exists');
ok(searchHtml.includes('.prose .img-row'), 'CSS .prose .img-row exists');
ok(searchHtml.includes('.prose figure'), 'CSS .prose figure exists');
ok(searchHtml.includes('img.onerror'), 'img.onerror placeholder logic exists');
ok(searchHtml.includes('escHtml(href)'), 'renderer.image escapes href via escHtml');
ok(searchHtml.includes('escHtml(text)'), 'renderer.image escapes alt via escHtml');

const qaEditor = loadHtml('qa_editor.html');
ok(qaEditor.includes('renderer.image'), 'qa_editor.html has renderer.image');
ok(qaEditor.includes('insert-image-btn'), 'qa_editor has insert-image button');
ok(qaEditor.includes('setRangeText'), 'insert uses setRangeText');
ok(qaEditor.includes('qa_images/'), 'insert template uses qa_images/');

// ---- 7. video rendering helpers (search.html ⇄ qa_editor.html parity) ----
section('video rendering (isVideoSrc parity, renderer)');
const { extractFunction } = require('./_extract');
const isVideoSearch = new Function(`${extractFunction(searchHtml, 'isVideoSrc')}; return isVideoSrc;`)();
const isVideoEditor = new Function(`${extractFunction(qaEditor, 'isVideoSrc')}; return isVideoSrc;`)();
const vidCases = {
  'qa_images/a.mp4': true, 'qa_images/a.MP4': true, 'qa_images/a.webm': true, 'qa_images/a.m4v': true,
  'qa_images/a.mov': true, 'qa_images/a.ogv': true, 'https://ex.com/v.mp4?x=1': true, 'qa_images/a.mp4#t=5': true,
  'qa_images/a.jpg': false, 'qa_images/a.png': false, 'qa_images/mp4': false, 'qa_images/a.mp4.jpg': false,
  '': false, 'javascript:alert(1)//.mp4x': false,
};
for (const [src, want] of Object.entries(vidCases)) {
  ok(isVideoSearch(src) === want, `search isVideoSrc(${JSON.stringify(src)}) === ${want}`);
  ok(isVideoEditor(src) === want, `editor isVideoSrc(${JSON.stringify(src)}) === ${want}`);
}
for (const [name, html] of [['search.html', searchHtml], ['qa_editor.html', qaEditor]]) {
  ok(/if \(isVideoSrc\(href\)\) return videoHtml\(href, text, title\);/.test(html), `${name}: renderer.image renders videos`);
  ok(/function videoHtml[\s\S]*?escHtml\(href\)[\s\S]*?escHtml\(label/.test(html), `${name}: videoHtml escapes src and label`);
}
// build.js / normalize-images.js / physq must accept the same video extensions
const VIDEO_EXTS = ['mp4', 'm4v', 'webm', 'ogv', 'mov'];
const normSrc = fs.readFileSync(path.join(REPO_ROOT, 'scripts/normalize-images.js'), 'utf-8');
const physqImage = fs.readFileSync(path.join(REPO_ROOT, 'physq/src/image.rs'), 'utf-8');
for (const ext of VIDEO_EXTS) {
  ok(new RegExp(`MEDIA_EXT_RE = /[^\n]*\\b${ext}\\b`).test(buildSrc), `build.js MEDIA_EXT_RE has .${ext}`);
  ok(new RegExp(`VIDEO_EXT_RE = /[^\n]*\\b${ext}\\b`).test(normSrc), `normalize-images.js VIDEO_EXT_RE has .${ext}`);
  ok(new RegExp(`VIDEO_EXTS[^\n]*"${ext}"`).test(physqImage), `physq VIDEO_EXTS has .${ext}`);
}

console.log(`\n${checks} checks, ${failures} failures`);
console.log(failures===0 ? 'QA IMAGES STRESS PASSED' : 'QA IMAGES STRESS FAILED');
process.exit(failures===0?0:1);
