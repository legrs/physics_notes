#!/usr/bin/env node
// media_editor.html のライセンス計算が、実際に配信される値（scripts/build.js
// の injectImageLicenses が q_and_a_data.json に書き込む image_licenses）と
// 一致することを、本物のデータで確かめる。ページ内の関数を抜き出して Node で
// 実行する（html_stress.js と同じ手法、依存なし）。
// Usage: node scripts/tests/media_editor_check.js
'use strict';

const fs = require('fs');
const path = require('path');
const { REPO_ROOT, loadHtml, extractFunction, extractConst } = require('./_extract');

let failures = 0;
let checks = 0;
function ok(cond, msg) {
  checks++;
  if (!cond) { failures++; console.log(`FAIL ${msg}`); }
}
function section(t) { console.log(`── ${t}`); }

const html = loadHtml('media_editor.html');
const FNS = ['isVideoSrc', 'isHeicName', 'extractMediaRefs', 'basenameOf', 'licenseKeysFor', 'resolveLicense',
  'captionText', 'isHttpUrl', 'cleanLicense', 'serializeLicenses', 'diffLicenses', 'buildItems', 'itemProblems', 'formatBytes',
  'isSafeMediaName', 'stagedName'];
const src = [extractConst(html, 'MEDIA_EXT_RE'), extractConst(html, 'UUID_NAME_RE'), ...FNS.map(n => extractFunction(html, n))].join('\n');
const W = new Function(`${src}; return { ${FNS.join(', ')} };`)();

const data = JSON.parse(fs.readFileSync(path.join(REPO_ROOT, 'q_and_a_data.json'), 'utf-8'));
const licenses = JSON.parse(fs.readFileSync(path.join(REPO_ROOT, 'qa_images/licenses.json'), 'utf-8'));
const buildSrc = fs.readFileSync(path.join(REPO_ROOT, 'scripts/build.js'), 'utf-8');
const buildExtract = new Function(`${buildSrc.match(/function extractImageSrcsForLicenses[\s\S]*?\n}\n/)[0]}; return extractImageSrcsForLicenses;`)();

section('parity with build.js on the real corpus');
let refs = 0;
for (const r of data) {
  const mine = W.extractMediaRefs(r.answer || '').map(x => x.src);
  const theirs = buildExtract(r.answer || '');
  ok(JSON.stringify(mine) === JSON.stringify(theirs), `refs of ${r.id} match build.js (${mine} vs ${theirs})`);
  for (const s of theirs) {
    refs++;
    // q_and_a_data.json は build.js が現在の licenses.json から生成した値を持つ。
    // ページの計算（licenses.json 個別指定 > 質問ごとの値 > _default）で同じ値になるはず。
    const got = W.resolveLicense(W.basenameOf(s), licenses, [r.image_licenses && r.image_licenses[s]]).lic;
    ok(JSON.stringify(got) === JSON.stringify(r.image_licenses[s]), `license of ${s} in ${r.id} matches build.js output`);
  }
}
ok(refs > 0, `corpus has media refs to compare (${refs})`);

section('precedence (licenses.json > record > _default)');
const L = { _default: { license: 'D' }, 'qa_images/a.jpg': { license: 'X' }, 'b.png': { license: 'Y' } };
ok(W.resolveLicense('a.jpg', L, [{ license: 'R' }]).source === 'explicit', 'qa_images/ key wins over record');
ok(W.resolveLicense('b.png', L, []).lic.license === 'Y', 'bare basename key also recognized (like build.js)');
ok(W.resolveLicense('c.jpg', L, [{ license: 'R' }]).lic.license === 'R', 'record value beats _default');
ok(W.resolveLicense('c.jpg', L, [{ license: '' }]).source === 'default', 'empty record license falls through to _default (build.js requires .license)');
ok(W.resolveLicense('c.jpg', {}, []).lic.license === 'Apache-2.0', 'no _default → Apache-2.0 like build.js');
ok(W.resolveLicense('c.jpg', L, [{ license: 'R' }, { license: 'S' }]).conflict === true, 'differing record values flagged');

section('extractMediaRefs edge cases');
const ex = W.extractMediaRefs('![a](qa_images/1.jpg) `![x](qa_images/no.jpg)`\n```\n<img src="qa_images/no2.jpg">\n```\n' +
  '<video poster="qa_images/p.jpg" title="振り子"><source src="qa_images/v.webm"></video> ![e](https://ex.com/e.png)');
ok(JSON.stringify(ex.map(e => e.src)) === JSON.stringify(['qa_images/1.jpg', 'qa_images/p.jpg', 'qa_images/v.webm']), `fences/external skipped, html media found: ${JSON.stringify(ex)}`);
ok(ex[0].alt === 'a', 'markdown alt kept');

section('caption text matches search.html enhanceImagesWithLicenses');
const cases = [
  [{ license: 'Apache-2.0' }, 'Apache-2.0'],
  [{ license: 'CC BY 4.0', attribution: '撮影: I' }, '撮影: I (CC BY 4.0)'],
  [{ license: 'Apache-2.0', attribution: '撮影: I' }, '撮影: I'],
  [{ license: '' }, ''],
  [{ attribution: 'A' }, 'A'],
];
for (const [lic, want] of cases) ok(W.captionText(lic) === want, `captionText(${JSON.stringify(lic)}) === ${JSON.stringify(want)}`);
// search.html の実装と同じ式であることも確認（どちらかを変えたら気づけるように）
const search = loadHtml('search.html');
ok(/if \(lic\.license && lic\.license !== 'Apache-2\.0'\) \{\s*capText = capText \? `\$\{capText\} \(\$\{lic\.license\}\)` : lic\.license;/.test(search),
  'search.html caption rule unchanged (update captionText in media_editor.html if this fails)');

section('licenses.json serialization / diff');
const base = { _default: { license: 'D' }, 'qa_images/z.jpg': { license: 'Z' }, 'qa_images/a.jpg': { license: 'A' } };
const draft = JSON.parse(JSON.stringify(base));
draft['qa_images/m.jpg'] = { license: 'M' };
draft['qa_images/b.jpg'] = { license: 'B' };
delete draft['qa_images/a.jpg'];
draft['qa_images/z.jpg'].attribution = 'x';
const out = W.serializeLicenses(base, draft);
ok(JSON.stringify(Object.keys(JSON.parse(out))) === JSON.stringify(['_default', 'qa_images/z.jpg', 'qa_images/b.jpg', 'qa_images/m.jpg']),
  `key order: _default, existing order, then new keys sorted: ${Object.keys(JSON.parse(out))}`);
ok(out.endsWith('}\n') && out.includes('\n  "_default"'), 'pretty-printed with trailing newline (same style as the repo file)');
const d = W.diffLicenses(base, draft);
ok(d.filter(x => x.type === 'add').length === 2 && d.filter(x => x.type === 'del').length === 1 && d.filter(x => x.type === 'mod').length === 1, 'diff counts add/del/mod');
ok(W.diffLicenses(licenses, JSON.parse(W.serializeLicenses(licenses, licenses))).length === 0, 'round trip of the real licenses.json is a no-op');
ok(JSON.stringify(W.cleanLicense({ license: ' L ', attribution: ' ', url: '' })) === '{"license":"L"}', 'cleanLicense trims and drops empty fields');
ok(W.isHttpUrl('https://a.b/c') && !W.isHttpUrl('javascript:alert(1)') && !W.isHttpUrl('//a.b'), 'only http(s) URLs are links');

section('library items');
const files = fs.readdirSync(path.join(REPO_ROOT, 'qa_images')).map(name => ({ name, size: 1 }));
const items = W.buildItems(files, data, licenses);
const mediaFiles = files.filter(f => /\.(jpe?g|png|webp|svg|gif|mp4|m4v|webm|ogv|mov|heic|heif)$/i.test(f.name));
ok(mediaFiles.every(f => items.some(i => i.name === f.name && i.exists)), 'every media file in qa_images/ is listed');
ok(!items.some(i => /licenses\.json|README|gitkeep/.test(i.name)), 'non-media files are not listed');
for (const it of items) {
  const probs = W.itemProblems(it, W.resolveLicense(it.name, licenses, it.uses.map(u => u.recordLicense)), true);
  ok(!probs.some(p => p.kind === 'err'), `${it.name}: no errors in the current repo (${probs.map(p => p.text)})`);
}
const synthetic = W.buildItems([{ name: 'IMG_1.HEIC', size: 5 }, { name: 'orphan.png', size: 5 }],
  [{ id: 'r', questions: ['q'], answer: '![x](qa_images/IMG_1.HEIC) ![y](qa_images/gone.jpg)' }], { 'qa_images/stale.jpg': { license: '' } });
const byName = Object.fromEntries(synthetic.map(i => [i.name, W.itemProblems(i, W.resolveLicense(i.name, { 'qa_images/stale.jpg': { license: '' } }, i.uses.map(u => u.recordLicense)), true).map(p => p.text)]));
ok(byName['IMG_1.HEIC'].includes('JPEG変換待ち'), 'HEIC flagged for conversion');
ok(byName['orphan.png'].includes('未使用') && byName['orphan.png'].includes('リネーム待ち'), 'unreferenced, non-UUID file flagged');
ok(byName['gone.jpg'].includes('ファイルなし'), 'reference to a missing file flagged');
ok(byName['stale.jpg'].includes('licenses.jsonのみ') && byName['stale.jpg'].includes('license空'), 'stale licenses.json key and empty license flagged');

section('local / added files');
ok(W.isSafeMediaName('photo_001.JPG') && W.isSafeMediaName('写真.heic') && W.isSafeMediaName('3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.mp4'), 'normal names are safe');
for (const bad of ['my photo.jpg', 'img(1).png', 'a#b.jpg', 'a%20b.jpg', 'noext', 'doc.pdf']) ok(!W.isSafeMediaName(bad), `unsafe: ${bad}`);
// the markdown regex the whole pipeline uses must be able to reference every "safe" name
const MD = /!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/;
for (const n of ['photo_001.JPG', '写真.heic', 'a-b.c.webm']) ok(MD.exec(`![x](qa_images/${n})`)[2] === `qa_images/${n}`, `safe name ${n} is referenceable`);
let u = 0;
const uuid = () => `00000000-0000-0000-0000-00000000000${u++}`;
ok(W.stagedName('photo.jpg', new Set(), uuid) === 'photo.jpg', 'safe, unused name kept (CI renames later)');
ok(/^0{8}-.*\.jpg$/.test(W.stagedName('my photo.JPG', new Set(), uuid)), 'unsafe name → UUID with lowercased ext');
ok(W.stagedName('photo.jpg', new Set(['photo.jpg']), uuid) !== 'photo.jpg', 'colliding name → UUID');
const loc = W.buildItems([{ name: 'new.png', size: 1, local: true, added: true, pushed: false }, { name: 'old.png', size: 1, local: true, pushed: true }], [], {});
const probsOf = n => W.itemProblems(loc.find(i => i.name === n), { source: 'default', lic: { license: 'x' } }, true).map(p => p.text);
ok(probsOf('new.png').includes('未push') && loc.find(i => i.name === 'new.png').added, 'added local file flagged 未push');
ok(!probsOf('old.png').includes('未push'), 'pushed local file not flagged');

console.log(`\n${checks} checks, ${failures} failures`);
console.log(failures === 0 ? 'MEDIA EDITOR CHECKS PASSED' : 'MEDIA EDITOR CHECKS FAILED');
process.exit(failures === 0 ? 0 : 1);
