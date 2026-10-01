#!/usr/bin/env node
// =============================================================
// Copyright 2026 Igarin & Legrs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// scripts/normalize-images.js
// QA画像ファイル名のUUID正規化
//
// 手元では雑な名前 (photo_001.jpg 等) で qa_images/ に置けば、CIが
// qa_images/<uuid>.jpg にリネームし、q_and_a_data.json 内参照も
// qa_images/licenses.json のキーも追従する。
//
// 画像だけでなく動画 (mp4/m4v/webm/ogv/mov) も同じ qa_images/ で扱う。
// iPhone 等の HEIC/HEIF は Chrome/Firefox で表示できないため、ここで JPEG に
// 変換してから UUID 名にする（元ファイルは削除、参照と licenses.json も追従）。
// 参照は ![説明](qa_images/x.mp4) か <video src> / <source src> / <video poster>。
//
// Usage:
//   node scripts/normalize-images.js          # リネーム + JSON書き換え
//   node scripts/normalize-images.js --check  # dry-run: 非UUID検出で非0終了
//   node scripts/normalize-images.js --root <dir>  # <dir>/qa_images と <dir>/q_and_a_data.json を対象にする（テスト用）
// =============================================================

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const rootArg = process.argv.indexOf('--root');
const REPO_ROOT = rootArg > 0 && process.argv[rootArg + 1]
  ? path.resolve(process.argv[rootArg + 1])
  : path.join(__dirname, '..');
const QA_IMAGES_DIR = path.join(REPO_ROOT, 'qa_images');
const DATA_PATH = path.join(REPO_ROOT, 'q_and_a_data.json');
const LICENSES_PATH = path.join(QA_IMAGES_DIR, 'licenses.json');

const CHECK = process.argv.includes('--check');

// UUID + 拡張子 (小文字) の正規形。_isUUID と同一正規表現を拡張子付きで流用
// https://github.com/... scripts/q&a_text_importer.gs:90
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.[a-z0-9]+$/;
// ベース名 (拡張子なし) が UUID かの判定
const UUID_BASE_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
// 画像 + 動画（build.js の MEDIA_EXT_RE / search.html の isVideoSrc と揃える）
const IMAGE_EXT_RE = /\.(jpe?g|png|webp|svg|gif|mp4|m4v|webm|ogv|mov)$/i;
const VIDEO_EXT_RE = /\.(mp4|m4v|webm|ogv|mov)$/i;
const VIDEO_WARN_BYTES = 20 * 1024 * 1024;
// 置けるが配信前に JPEG へ変換する形式
const HEIC_EXT_RE = /\.(heic|heif)$/i;
// HTML の画像・動画タグと、その中で qa_images/ を指しうる属性
const HTML_MEDIA_RE = /<(?:img|video|source)\b[^>]*>/gi;
const MEDIA_ATTRS = ['src', 'poster'];
function tagAttr(tag, attr) {
  const m = tag.match(new RegExp(`\\s${attr}\\s*=\\s*(['"])(.*?)\\1`, 'i'))
    || tag.match(new RegExp(`\\s${attr}\\s*=\\s*([^\\s>]+)`, 'i'));
  return m ? (m[2] || m[1]) : null;
}

function isUuidFilename(name) {
  return UUID_RE.test(name);
}

// コードフェンス/インラインコードを一時置換して誤検出を防ぐ
function stripCodeFencesForScan(s) {
  // placeholder は衝突しにくい文字列
  const fences = [];
  let idx = 0;
  const placeholder = (i) => `__CODE_FENCE_${i}__`;
  let out = s.replace(/```[\s\S]*?```/g, (m) => {
    fences.push(m);
    return placeholder(idx++);
  });
  out = out.replace(/`[^`]*`/g, (m) => {
    fences.push(m);
    return placeholder(idx++);
  });
  return out;
}

// markdown 画像の src 抽出 (コードフェンス除去後)
// spec: /!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g
const MD_IMG_RE = /!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g;
function extractImgSrcs(answer) {
  const srcs = [];
  const cleaned = stripCodeFencesForScan(answer);
  let m;
  MD_IMG_RE.lastIndex = 0;
  while ((m = MD_IMG_RE.exec(cleaned)) !== null) {
    srcs.push(m[2]);
  }
  // HTML (<img src> / <video src|poster> / <source src>)
  HTML_MEDIA_RE.lastIndex = 0;
  while ((m = HTML_MEDIA_RE.exec(cleaned)) !== null) {
    for (const attr of MEDIA_ATTRS) {
      const src = tagAttr(m[0], attr);
      if (src) srcs.push(src);
    }
  }
  return srcs;
}

// HEIC/HEIF → JPEG。sharp のプリビルドは HEVC を復号できない（AVIF のみ）ため
// heic-decode（libheif の WASM 版）で復号し、sharp で JPEG にする。生の画素から
// 作り直すので、iPhone 写真の位置情報などの EXIF もここで落ちる。向き（回転）は
// libheif が復号時に適用済み。透過は白で塗りつぶす。
async function heicToJpeg(buf) {
  const sharp = require('sharp');
  let img;
  try {
    const decode = require('heic-decode');
    const { width, height, data } = await decode({ buffer: buf });
    img = sharp(Buffer.from(data.buffer, data.byteOffset, data.byteLength), { raw: { width, height, channels: 4 } });
  } catch (e) {
    // AV1 で符号化された .heif 等は sharp 自身が読める
    img = sharp(buf);
  }
  return img.flatten({ background: '#ffffff' }).jpeg({ quality: 90, mozjpeg: true }).toBuffer();
}

async function convertHeicFiles(renameMap) {
  let converted = 0;
  const heics = fs.readdirSync(QA_IMAGES_DIR).filter((f) => HEIC_EXT_RE.test(f));
  for (const name of heics) {
    let newName;
    do { newName = crypto.randomUUID() + '.jpg'; } while (fs.existsSync(path.join(QA_IMAGES_DIR, newName)));
    if (CHECK) {
      renameMap.set(name, newName);
      console.log(`[check] Would convert ${name} -> ${newName} (JPEG)`);
      continue;
    }
    try {
      const jpg = await heicToJpeg(fs.readFileSync(path.join(QA_IMAGES_DIR, name)));
      fs.writeFileSync(path.join(QA_IMAGES_DIR, newName), jpg);
      fs.unlinkSync(path.join(QA_IMAGES_DIR, name));
      renameMap.set(name, newName);
      converted++;
      console.log(`Converted ${name} -> ${newName} (JPEG, ${(jpg.length / 1024).toFixed(0)} KB)`);
    } catch (e) {
      console.warn(`⚠ Failed to convert ${name}: ${e.message} — left as is (it will not display in Chrome/Firefox)`);
    }
  }
  return converted;
}

async function main() {
  let renamed = 0;
  let updatedRefs = 0;
  const renameMap = new Map(); // old basename -> new basename
  const extNormalizeMap = new Map(); // for .JPG -> .jpg even if UUID basename matches

  // 1) scan qa_images/*
  if (!fs.existsSync(QA_IMAGES_DIR)) {
    console.log('No qa_images directory — nothing to do.');
    if (CHECK) process.exit(0);
    return;
  }

  // 0) HEIC/HEIF → <uuid>.jpg（renameMap に載せるので、以降の参照書き換え・
  //    licenses.json のキー追従は通常のリネームと同じ経路で行われる）
  renamed += await convertHeicFiles(renameMap);

  const files = fs.readdirSync(QA_IMAGES_DIR).filter((f) => {
    // licenses.json / .gitkeep / README.md はスキップ、画像のみ
    if (f === 'licenses.json' || f === '.gitkeep' || f === 'README.md') return false;
    // ディレクトリは除外
    try {
      if (fs.statSync(path.join(QA_IMAGES_DIR, f)).isDirectory()) return false;
    } catch (_) {
      return false;
    }
    return IMAGE_EXT_RE.test(f);
  });

  // 外部URLは scan 対象外なので files には入らない (念のため data: 等除外)
  for (const oldName of files) {
    const ext = path.extname(oldName);
    if (!ext) {
      console.warn(`⚠ ${oldName} has no extension — skipping`);
      continue;
    }
    const extLower = ext.toLowerCase();
    const base = path.basename(oldName, ext);
    const newExt = extLower;
    const baseLower = base; // UUID は小文字想定、basename はそのまま判定

    // 拡張子小文字化が必要な場合でも、basename がUUIDならリネームではなく拡張子のみ修正扱い
    // しかし spec は「basename がUUID正規形に不一致のもののみ」対象なので、
    // UUID basename だが拡張子が大文字のケースは別途 extNormalizeMap で扱う
    const lowerName = base + newExt;
    // 既に正規形か判定: 小文字化後の名前がUUID_REにマッチするか
    if (isUuidFilename(lowerName) && oldName === lowerName) {
      continue; // 正規形、スキップ
    }
    // basename がUUID形式かつ拡張子だけ違う場合は extNormalize 用に記録するが、renameMapにも入れる
    // 非UUID basename はUUIDにリネーム
    const isUuidBase = UUID_BASE_RE.test(base.toLowerCase()) && base === base.toLowerCase();
    // Note: base の大文字小文字も正規形では小文字のみ許可。UUID_BASE_RE は小文字のみ。
    // 大文字UUIDは非正規だが、内容保持を優先し小文字化で正規化する（新規UUIDを振り直すと
    // 既存の answer 参照と licenses.json の対応が失われる）。衝突時のみ新規UUIDへフォールバック。
    // 小文字UUIDで拡張子だけ大文字のケースも同様に拡張子小文字化のみで済ませ、renameMap経由で追従する。

    if (isUuidBase && oldName !== lowerName && isUuidFilename(lowerName)) {
      // 拡張子のみ正規化ケース: 同じUUIDで拡張子を小文字化
      // 衝突チェック: lowerName が既存ファイルと衝突しないか
      if (fs.existsSync(path.join(QA_IMAGES_DIR, lowerName)) && lowerName !== oldName) {
        console.warn(`⚠ ${oldName} -> ${lowerName} would collide — generating new UUID instead`);
        // fall through to UUID generation
      } else {
        if (CHECK) {
          renameMap.set(oldName, lowerName);
          extNormalizeMap.set(oldName, lowerName);
          continue;
        }
        try {
          fs.renameSync(path.join(QA_IMAGES_DIR, oldName), path.join(QA_IMAGES_DIR, lowerName));
          renameMap.set(oldName, lowerName);
          extNormalizeMap.set(oldName, lowerName);
          renamed++;
          console.log(`Renamed ${oldName} -> ${lowerName} (ext normalize)`);
        } catch (e) {
          console.warn(`⚠ Failed to rename ${oldName} -> ${lowerName}: ${e.message} — skipping JSON rewrite for this file`);
        }
        continue;
      }
    }

    // 非UUID -> UUID生成
    if (!isUuidFilename(oldName) && !isUuidFilename(lowerName)) {
      // 新UUID生成、衝突時は再生成
      let newName;
      let attempts = 0;
      do {
        const uuid = crypto.randomUUID();
        newName = uuid + newExt;
        attempts++;
        if (attempts > 10) {
          console.warn(`⚠ Could not generate unique UUID for ${oldName} after 10 attempts — skipping`);
          newName = null;
          break;
        }
      } while (fs.existsSync(path.join(QA_IMAGES_DIR, newName)));
      if (!newName) continue;
      if (CHECK) {
        renameMap.set(oldName, newName);
        continue;
      }
      try {
        fs.renameSync(path.join(QA_IMAGES_DIR, oldName), path.join(QA_IMAGES_DIR, newName));
        renameMap.set(oldName, newName);
        renamed++;
        console.log(`Renamed ${oldName} -> ${newName}`);
      } catch (e) {
        console.warn(`⚠ Failed to rename ${oldName} -> ${newName}: ${e.message} — skipping JSON rewrite for this file`);
        // renameMapに記録せず、後段のJSON書き換えをスキップ
      }
      continue;
    }

    // それ以外で oldName !== lowerName だが lowerName がUUID_REにマッチする場合 (例: 大文字UUID + 小文字拡張子不整合)
    // 上で処理済みだが、漏れがあればここで拡張子正規化
    if (oldName !== lowerName && isUuidFilename(lowerName)) {
      if (CHECK) {
        renameMap.set(oldName, lowerName);
        continue;
      }
      if (fs.existsSync(path.join(QA_IMAGES_DIR, lowerName)) && lowerName !== oldName) {
        console.warn(`⚠ ${oldName} -> ${lowerName} collision — skipping`);
        continue;
      }
      try {
        fs.renameSync(path.join(QA_IMAGES_DIR, oldName), path.join(QA_IMAGES_DIR, lowerName));
        renameMap.set(oldName, lowerName);
        renamed++;
        console.log(`Renamed ${oldName} -> ${lowerName}`);
      } catch (e) {
        console.warn(`⚠ Failed to rename ${oldName} -> ${lowerName}: ${e.message}`);
      }
    }
  }

  // 2) load q_and_a_data.json and rewrite refs
  let data;
  try {
    data = JSON.parse(fs.readFileSync(DATA_PATH, 'utf-8'));
  } catch (e) {
    console.error(`❌ Failed to load ${DATA_PATH}: ${e.message}`);
    process.exit(1);
  }

  let jsonChanged = false;
  // 全参照 src の収集 (orphan/broken 検出用)
  const allReferencedBasenames = new Set();
  const allReferencedSrcs = [];

  for (const item of data) {
    if (!item.answer || typeof item.answer !== 'string') continue;
    const srcs = extractImgSrcs(item.answer);
    for (const s of srcs) {
      if (s.startsWith('qa_images/')) {
        const bn = path.basename(s);
        allReferencedBasenames.add(bn);
        allReferencedSrcs.push(s);
      }
    }
  }

  // 置換処理
  for (const item of data) {
    if (!item.answer || typeof item.answer !== 'string') continue;
    let newAnswer = item.answer;
    let itemChanged = false;

    // コードフェンスを一時置換してから走査し、置換は元文字列に対して行う
    // 単純な置換ではコードフェンス内の ![]() を誤って書き換える恐れがあるため、
    // コードフェンス除去後の検出結果を元に、元文字列の qa_images/ 参照を置換する。
    // ここでは「コードフェンス外の参照のみ」を対象にするため、フェンス部分を除外して処理。

    // フェンス部分の範囲を特定して、その外側のみ置換する簡易実装:
    // 元の answer をフェンスで分割し、フェンス外のチャンクのみで置換を行う。
    const parts = [];
    let lastIdx = 0;
    const fenceRe = /```[\s\S]*?```|`[^`]*`/g;
    let fm;
    const fenceRanges = [];
    while ((fm = fenceRe.exec(item.answer)) !== null) {
      fenceRanges.push([fm.index, fm.index + fm[0].length]);
    }
    function isInFence(pos) {
      for (const [s, e] of fenceRanges) if (pos >= s && pos < e) return true;
      return false;
    }

    // markdown 置換
    // グローバル置換だが、フェンス内はスキップするため手動ループ
    const mdRe = /!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g;
    let mdMatch;
    // 置換を後ろから行うとインデックスがずれない
    const mdReplacements = [];
    while ((mdMatch = mdRe.exec(item.answer)) !== null) {
      if (isInFence(mdMatch.index)) continue;
      const src = mdMatch[2];
      // qa_images/ 相対のみ対象
      if (!src.startsWith('qa_images/')) continue;
      // 外部URLはスキップ (https://, data:, //) — qa_images/ で始まるものは相対なので外部はここで除外済み
      if (/^(https?:)?\/\//i.test(src) || src.startsWith('data:')) continue;
      const bn = path.basename(src);
      const ext = path.extname(bn);
      const extLower = ext.toLowerCase();
      const bnLower = ext !== extLower ? bn.slice(0, -ext.length) + extLower : bn;
      let newSrc = null;
      if (renameMap.has(bn)) {
        newSrc = 'qa_images/' + renameMap.get(bn);
      } else if (renameMap.has(bnLower) && bn !== bnLower) {
        // 拡張子正規化前のbasenameでマップされている場合
        newSrc = 'qa_images/' + renameMap.get(bnLower);
      } else if (bn !== bnLower) {
        // renameMapにないが拡張子大文字のケース: 小文字化のみ
        // ただしファイルが実際にリネームされたか、またはCHECKモードで記録された場合のみ
        // ここでは拡張子小文字化も正規化として扱う
        const lowerSrc = 'qa_images/' + bnLower;
        // bnLowerがUUID形式で、ファイル側でリネーム済み or マップにあるか確認
        // 単純に小文字化して良い: .JPG -> .jpg は常に正規化対象
        newSrc = lowerSrc;
      }
      if (newSrc && newSrc !== src) {
        mdReplacements.push({ index: mdMatch.index, len: mdMatch[0].length, alt: mdMatch[1], src, newSrc, full: mdMatch[0] });
      }
    }
    // Markdown と HTML の置換位置はどちらも元の item.answer 基準なので、
    // 両方を集めてから位置の降順に1回で適用する（以前は Markdown 側を先に
    // 適用して長さが変わった後に、HTML 側を古い位置で適用しており、同じ
    // answer に両方の記法があると本文が壊れていた）
    const replacements = mdReplacements.map((r) => {
      // altはそのまま、srcのみ置換。title付きの場合は title を保持
      const titleMatch = r.full.match(/!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"([^"]*)")?\s*\)/);
      const title = titleMatch && titleMatch[3] ? ` "${titleMatch[3]}"` : '';
      return { index: r.index, len: r.len, text: `![${r.alt}](${r.newSrc}${title})` };
    });

    // HTML <img> / <video> / <source> 置換（src と poster）
    const htmlRe = new RegExp(HTML_MEDIA_RE.source, 'gi');
    let htmlMatch;
    const htmlReplacements = [];
    while ((htmlMatch = htmlRe.exec(item.answer)) !== null) {
      if (isInFence(htmlMatch.index)) continue;
      const tag = htmlMatch[0];
      let newTag = tag;
      for (const attr of MEDIA_ATTRS) {
        const src = tagAttr(tag, attr);
        if (!src || !src.startsWith('qa_images/')) continue;
        if (/^(https?:)?\/\//i.test(src) || src.startsWith('data:')) continue;
        const bn = path.basename(src);
        const ext = path.extname(bn);
        const extLower = ext.toLowerCase();
        const bnLower = ext !== extLower ? bn.slice(0, -ext.length) + extLower : bn;
        let newSrc = null;
        if (renameMap.has(bn)) {
          newSrc = 'qa_images/' + renameMap.get(bn);
        } else if (bn !== bnLower) {
          newSrc = 'qa_images/' + bnLower;
        }
        // 属性値だけを置換（同じ文字列がタグ内の別の場所にあっても触らない）
        if (newSrc && newSrc !== src) {
          newTag = newTag.replace(
            new RegExp(`(\\s${attr}\\s*=\\s*['"]?)${src.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`, 'i'),
            (_, pre) => pre + newSrc
          );
        }
      }
      if (newTag !== tag) {
        htmlReplacements.push({ index: htmlMatch.index, len: tag.length, newTag });
      }
    }
    for (const r of htmlReplacements) replacements.push({ index: r.index, len: r.len, text: r.newTag });
    replacements.sort((a, b) => b.index - a.index);
    for (const r of replacements) {
      newAnswer = newAnswer.slice(0, r.index) + r.text + newAnswer.slice(r.index + r.len);
      itemChanged = true;
      updatedRefs++;
    }

    if (itemChanged) {
      item.answer = newAnswer;
      jsonChanged = true;
    }
  }

  // 4) licenses.json のキーもリネーム
  let licensesChanged = false;
  let licensesData = null;
  if (fs.existsSync(LICENSES_PATH)) {
    try {
      licensesData = JSON.parse(fs.readFileSync(LICENSES_PATH, 'utf-8'));
    } catch (e) {
      console.warn(`⚠ Failed to parse ${LICENSES_PATH}: ${e.message} — skipping licenses rewrite`);
      licensesData = null;
    }
    if (licensesData && typeof licensesData === 'object') {
      const newLicenses = {};
      for (const [key, val] of Object.entries(licensesData)) {
        if (key === '_default') {
          newLicenses[key] = val;
          continue;
        }
        // 外部URLキーは対象外
        if (/^(https?:)?\/\//i.test(key) || key.startsWith('data:')) {
          newLicenses[key] = val;
          continue;
        }
        // キーが qa_images/ プレフィックス付きの場合も考慮? spec は拡張子込みUUIDファイル名のみ
        // ただし qa_images/xxx.jpg 形式で書かれている場合もbnで判定
        const bn = path.basename(key);
        if (renameMap.has(bn)) {
          const newKey = renameMap.get(bn);
          // キーが qa_images/付きならプレフィックス保持
          const isPrefixed = key.startsWith('qa_images/');
          const finalKey = isPrefixed ? 'qa_images/' + newKey : newKey;
          // 拡張子小文字化も考慮: newKey は既に小文字
          if (finalKey !== key) licensesChanged = true;
          newLicenses[finalKey] = val;
        } else {
          // 拡張子小文字化のみ
          const ext = path.extname(bn);
          const extLower = ext.toLowerCase();
          if (ext !== extLower) {
            const bnLower = bn.slice(0, -ext.length) + extLower;
            const newKey = key.replace(bn, bnLower);
            if (newKey !== key) licensesChanged = true;
            newLicenses[newKey] = val;
          } else {
            newLicenses[key] = val;
          }
        }
      }
      if (licensesChanged) jsonChanged = true;
      licensesData = newLicenses;
    }
  }

  // 5) 書き戻し
  if (CHECK) {
    if (renameMap.size > 0 || updatedRefs > 0 || licensesChanged) {
      console.log(`[check] Would rename ${renameMap.size} files, update ${updatedRefs} refs${licensesChanged ? ', update licenses.json' : ''}`);
      if (renameMap.size) {
        for (const [o, n] of renameMap) console.log(`  ${o} -> ${n}`);
      }
      process.exit(1);
    } else {
      console.log('[check] All qa_images filenames are normalized.');
      process.exit(0);
    }
  }

  if (jsonChanged) {
    try {
      JSON.parse(JSON.stringify(data)); // validation
      fs.writeFileSync(DATA_PATH, JSON.stringify(data, null, 4) + '\n', 'utf-8');
      console.log(`Updated ${DATA_PATH} (${updatedRefs} refs)`);
    } catch (e) {
      console.error(`❌ Failed to write ${DATA_PATH}: ${e.message}`);
      process.exit(1);
    }
  }

  if (licensesChanged && licensesData) {
    try {
      JSON.parse(JSON.stringify(licensesData));
      fs.writeFileSync(LICENSES_PATH, JSON.stringify(licensesData, null, 2) + '\n', 'utf-8');
      console.log(`Updated ${LICENSES_PATH}`);
    } catch (e) {
      console.error(`❌ Failed to write ${LICENSES_PATH}: ${e.message}`);
      process.exit(1);
    }
  }

  // 6) レポート: orphan / broken
  // orphan: qa_images/ にあるが参照されていないファイル
  const currentFiles = fs.existsSync(QA_IMAGES_DIR)
    ? fs.readdirSync(QA_IMAGES_DIR).filter((f) => IMAGE_EXT_RE.test(f))
    : [];
  // 参照セットは再計算 (リネーム後の新basenameで)
  const referencedAfter = new Set();
  for (const item of data) {
    if (!item.answer) continue;
    for (const s of extractImgSrcs(item.answer)) {
      if (s.startsWith('qa_images/')) referencedAfter.add(path.basename(s));
    }
  }
  const orphans = currentFiles.filter((f) => !referencedAfter.has(f));
  const broken = [...referencedAfter].filter((bn) => !currentFiles.includes(bn));

  console.log(`\nRenamed ${renamed} files, updated ${updatedRefs} refs`);
  if (orphans.length) console.log(`Orphan files (no refs): ${orphans.join(', ')}`);
  else console.log('No orphan files');
  if (broken.length) console.log(`Broken refs (missing files): ${broken.join(', ')}`);
  else console.log('No broken refs');

  // 動画の注意（reject はしない）
  const MB = 1024 * 1024;
  for (const f of currentFiles.filter((f) => VIDEO_EXT_RE.test(f))) {
    const size = fs.statSync(path.join(QA_IMAGES_DIR, f)).size;
    if (size >= VIDEO_WARN_BYTES) {
      console.log(`⚠ ${f} is ${(size / MB).toFixed(1)}MB — 動画は ${VIDEO_WARN_BYTES / MB}MB 未満を推奨（GitHub は 100MB 超を拒否、50MB 超で警告）`);
    }
    if (/\.mov$/i.test(f)) {
      console.log(`⚠ ${f}: .mov（特に iPhone の HEVC）は Chrome/Firefox で再生できないことがあります。H.264 の .mp4 を推奨`);
    }
  }

  // licenses.json 未記載の警告 (Apache-2.0扱い)
  if (licensesData) {
    const licensedSet = new Set(Object.keys(licensesData).filter((k) => k !== '_default').map((k) => path.basename(k)));
    const unlicensed = currentFiles.filter((f) => !licensedSet.has(f));
    if (unlicensed.length) console.log(`Unlicensed (default Apache-2.0): ${unlicensed.join(', ')}`);
  }
}

main().catch((e) => {
  console.error(`❌ ${e && e.stack || e}`);
  process.exit(1);
});
