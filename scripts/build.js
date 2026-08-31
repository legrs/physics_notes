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
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
// =============================================================
// scripts/build.js
// 1. search_text 自動生成（kuromoji）
// 2. Embedding 生成（multilingual-e5-small / large）
//
// 使い方:
//   node scripts/build.js              # search_text のみ
//   node scripts/build.js --embed      # search_text + Embedding 生成
//   node scripts/build.js --embed-only # Embedding のみ（search_text はスキップ）
//   node scripts/build.js --data <path>
//       # 任意のデータファイルの search_text を再生成（自己改善ループ
//       # scripts/self_improve.py の作業コピー用）。リポジトリ正本の
//       # embeddings.json / version.json には一切触れないため、
//       # --embed とは併用不可で version.json も生成しない。
// =============================================================

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const kuromoji = require('kuromoji');

const dataIdx = process.argv.indexOf('--data');
const CUSTOM_DATA = dataIdx !== -1 ? process.argv[dataIdx + 1] : null;
if (dataIdx !== -1 && !CUSTOM_DATA) {
  console.error('❌ --data にはファイルパスを指定してください');
  process.exit(1);
}

const JSON_PATH = CUSTOM_DATA
  ? path.resolve(CUSTOM_DATA)
  : path.join(__dirname, '..', 'q_and_a_data.json');
const EMBEDDINGS_PATH = path.join(__dirname, '..', 'embeddings.json');
const VERSION_PATH = path.join(__dirname, '..', 'version.json');

const DO_EMBED = process.argv.includes('--embed') || process.argv.includes('--embed-only');
const SKIP_TEXT = process.argv.includes('--embed-only');

if (CUSTOM_DATA && DO_EMBED) {
  console.error('❌ --data と --embed/--embed-only は併用できません（embeddings.json はリポジトリ正本のみ）');
  process.exit(1);
}

// ── モデル定義 ───────────────────────────────────────────────
const MODELS = {
  small: { id: 'Xenova/multilingual-e5-small' },
  large: { id: 'Xenova/multilingual-e5-large' },
};

// ── version.json スキーマ定数 ─────────────────────────────
// physq (CLI) の CLAUDE.md §3/§5/§8 で決め打ちされている値と揃える。
// tokenizer が変わったら physq 側の BM25 インデックスキャッシュが再構築される。
// schema_version 4: qa_images (combined hash) 追加 (2026-08-31)
const VERSION_SCHEMA = 4;
const TOKENIZER_TAG = 'lindera-ipadic';
// embeddings.json の実際のキー（"small"/"large"）ごとのモデル名。MODELS から導出し、
// 二重管理を避ける。
const EMBEDDING_MODEL_TAGS = Object.fromEntries(
  Object.entries(MODELS).map(([key, m]) => [key, m.id.replace(/^Xenova\//, '')])
);

// ── ID 正規化 ───────────────────────────────────────────────
// search.html / q&a_text_importer.gs と同じ規則。
// 数値 ID は先頭ゼロを除去（"00001" → "1"）、UUID 等の非数値 ID は
// そのまま返す（将来の UUID 移行に対応）。embeddings のキーを正規形に
// そろえることで、保存形式に依存せず検索側と一致させる。
function normalizeId(id) {
  if (id == null) return '';
  const s = String(id).trim();
  return /^\d+$/.test(s) ? s.replace(/^0+(?=\d)/, '') : s;
}

// ── LaTeX 除去 ──────────────────────────────────────────────
function stripLatex(str) {
  return str
    .replace(/\$\$[\s\S]*?\$\$/g, ' ')
    .replace(/\$[^$\n]+?\$/g, ' ')
    .replace(/\\[a-zA-Z]+\{[^}]*\}/g, ' ')
    .replace(/\\[a-zA-Z]+/g, ' ')
    .replace(/[{}^_]/g, ' ')
    .replace(/\s+/g, ' ').trim();
}

// ── 画像記法除去（search_text 用） ─────────────────────────
// §5: altのみ残し srcは除去。コードブロック内の ![]() は無視、title付きや <img alt> も対応
function stripCodeFences(s) { return s.replace(/```[\s\S]*?```/g,' ').replace(/`[^`]*`/g,' '); }
function stripMarkdownImages(s){ return s.replace(/!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g, ' $1 '); }
function stripHtmlImages(s){ return s.replace(/<img\b[^>]*>/gi, m => { const alt=(m.match(/alt\s*=\s*(['"])(.*?)\1/i)||[])[2]||''; return ' '+alt+' '; }); }
function stripHtmlImageRows(s){ return s.replace(/<div class="img-row">/g,' ').replace(/<\/div>/g,' '); }

// ── カタカナ → ひらがな ────────────────────────────────────
function toHiragana(str) {
  return str.replace(/[\u30a1-\u30f6]/g, ch =>
    String.fromCharCode(ch.charCodeAt(0) - 0x60));
}

// ── kuromoji 読み取得 ─────────────────────────────────────
function getReading(tokenizer, text) {
  return tokenizer.tokenize(text)
    .map(t => t.reading || t.surface_form).join('');
}

// ── search_text 生成 ──────────────────────────────────────
// BM25はスペース区切りのトークン単位で動作するため、
// 句読点などを除いた形態素（surface_form）も個別に追加する
function buildSearchText(tokenizer, item) {
  // category は複数割り当て可（配列）。旧データの文字列形式にも後方互換で対応する。
  const categories = Array.isArray(item.category)
    ? item.category
    : (item.category ? [item.category] : []);
  const fields = [
    ...(item.questions || []),
    item.answer || '',
    item.description || '',
    ...(item.keywords || []),
    ...(item.synonyms || []),
    ...categories,
  ];
  // 画像記法は alt のみ残し src は除去（§5）。コードブロック内の ![]() は無視
  let rawJoined = fields.map(s => String(s)).filter(Boolean).join(' ');
  rawJoined = stripCodeFences(rawJoined);
  rawJoined = stripMarkdownImages(rawJoined);
  rawJoined = stripHtmlImages(rawJoined);
  rawJoined = stripHtmlImageRows(rawJoined);
  const cleaned = stripLatex(rawJoined).replace(/\s+/g, ' ').trim();

  // 形態素に分解してスペース区切りで追加（BM25用）
  // 記号・助詞1文字などは除外してノイズを減らす
  const morphemes = tokenizer.tokenize(cleaned)
    .map(t => t.surface_form.trim())
    .filter(t => t.length >= 2 || /[a-zA-Z0-9]/.test(t))  // 2文字未満の日本語記号は除外
    .join(' ');

  // 読み（カタカナ・ひらがな）
  const kata = getReading(tokenizer, cleaned);
  const hira = toHiragana(kata);

  const parts = new Set([cleaned, morphemes]);
  if (kata !== cleaned) parts.add(kata);
  if (hira !== cleaned && hira !== kata) parts.add(hira);
  return [...parts].join(' ').replace(/\s+/g, ' ').trim();
}

// ── Embedding 生成 ─────────────────────────────────────────
// e5 モデルは文書に "passage: " プレフィックスが必要
async function buildEmbeddings(data) {
  const os = require('os');
  const { pipeline, env } = await import('@xenova/transformers');

  // デフォルトでは node_modules/@xenova/transformers/.cache/ に保存されてしまい、
  // GitHub Actions の actions/cache でキャッシュしづらいため、
  // ホームディレクトリ配下の分かりやすい場所に明示的に変更する
  env.cacheDir = path.join(os.homedir(), '.cache', 'huggingface', 'transformers-js');

  const existing = fs.existsSync(EMBEDDINGS_PATH)
    ? JSON.parse(fs.readFileSync(EMBEDDINGS_PATH, 'utf-8'))
    : {};

  // 現在のデータに存在する（正規化済み）ID 集合。
  // ID 移行（数値 → UUID）後に残る旧キーを掃除するために使う。
  const validIds = new Set(data.map(item => normalizeId(item.id)));

  for (const [key, model] of Object.entries(MODELS)) {
    console.log(`\n📐 ${key}（${model.id}）のEmbeddingを生成中...`);
    const extractor = await pipeline('feature-extraction', model.id, { quantized: true });

    if (!existing[key]) existing[key] = {};
    let updated = 0;

    for (let i = 0; i < data.length; i++) {
      const item = data[i];

      const text = `passage: ${item.questions[0]} ${item.description}`;
      const out = await extractor(text, { pooling: 'mean', normalize: true });
      existing[key][normalizeId(item.id)] = Array.from(out.data);
      updated++;

      if ((i + 1) % 10 === 0 || i === data.length - 1) {
        process.stdout.write(`  ${i + 1}/${data.length} 件完了\r`);
      }
    }

    // 現在のデータに無いキー（ID 移行前の旧 ID など）を除去
    let pruned = 0;
    for (const k of Object.keys(existing[key])) {
      if (!validIds.has(k)) { delete existing[key][k]; pruned++; }
    }

    console.log(`  ✅ ${key}: ${updated} 件更新` + (pruned ? `, ${pruned} 件の旧キーを削除` : ''));
  }

  fs.writeFileSync(EMBEDDINGS_PATH, JSON.stringify(existing), 'utf-8');
  console.log(`\n💾 embeddings.json を保存しました`);
}

// ── version.json 生成 ─────────────────────────────────────
// 配信するデータファイルをハッシュ化して物理・CLI 両方が参照する共通の
// マニフェストを作る（CLAUDE.md §5）。ハッシュアルゴリズムは物理・CLI 間で
// 一致している必要は無く（CLI 側は不透明な文字列として比較するだけ）、
// SHA-256 を採用する。
function fileManifest(filePath) {
  const buf = fs.readFileSync(filePath);
  const hash = crypto.createHash('sha256').update(buf).digest('hex');
  return { hash, size: buf.length };
}

function qaImagesManifest() {
  const dir = path.join(__dirname, '..', 'qa_images');
  if (!fs.existsSync(dir)) return null;
  const IMAGE_EXT_RE = /\.(jpe?g|png|webp|svg|gif)$/i;
  const files = fs.readdirSync(dir).filter(f => {
    if (f === 'licenses.json' || f === '.gitkeep' || f === 'README.md') return false;
    try { if (fs.statSync(path.join(dir, f)).isDirectory()) return false; } catch (_) { return false; }
    return IMAGE_EXT_RE.test(f);
  });
  files.sort();
  let total = 0;
  const buckets = { lt3: 0, btw: 0, gte5: 0 };
  const hashes = [];
  for (const f of files) {
    const p = path.join(dir, f);
    const buf = fs.readFileSync(p);
    total += buf.length;
    hashes.push(crypto.createHash('sha256').update(buf).digest('hex'));
    const mb = buf.length / (1024 * 1024);
    if (mb < 3) buckets.lt3++;
    else if (mb < 5) buckets.btw++;
    else buckets.gte5++;
  }
  const combined = hashes.length ? crypto.createHash('sha256').update(hashes.join('')).digest('hex') : crypto.createHash('sha256').update('').digest('hex');
  return {
    hash: combined,
    count: files.length,
    total_bytes: total,
    avg_bytes: files.length ? Math.round(total / files.length) : 0,
    buckets,
  };
}

function generateVersionManifest() {
  const manifest = {
    generated_at: new Date().toISOString(),
    schema_version: VERSION_SCHEMA,
    tokenizer: TOKENIZER_TAG,
    embedding_model: EMBEDDING_MODEL_TAGS,
    files: {
      'q_and_a_data.json': fileManifest(JSON_PATH),
      'embeddings.json': fileManifest(EMBEDDINGS_PATH),
    },
  };
  const qa = qaImagesManifest();
  if (qa) manifest.qa_images = qa;
  fs.writeFileSync(VERSION_PATH, JSON.stringify(manifest, null, 2) + '\n', 'utf-8');
  console.log('✅ version.json 生成完了' + (qa ? ` (qa_images: ${qa.count} files, ${qa.hash.slice(0,8)}…)` : ''));
}

// ── image_licenses 注入（§12-5 C: ビルド時埋め込み） ─────────────
function loadLicensesMap() {
  const p = path.join(__dirname, '..', 'qa_images', 'licenses.json');
  if (!fs.existsSync(p)) return {};
  try { return JSON.parse(fs.readFileSync(p, 'utf-8')); } catch (e) { console.warn(`⚠ licenses.json parse failed: ${e.message}`); return {}; }
}
function extractImageSrcsForLicenses(answer) {
  if (!answer || typeof answer !== 'string') return [];
  const fenceRe = /```[\s\S]*?```|`[^`]*`/g;
  const ranges = [];
  let m;
  while ((m = fenceRe.exec(answer)) !== null) ranges.push([m.index, m.index + m[0].length]);
  const inFence = (pos) => ranges.some(([s, e]) => pos >= s && pos < e);
  const srcs = [];
  const mdRe = /!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g;
  while ((m = mdRe.exec(answer)) !== null) {
    if (inFence(m.index)) continue;
    const src = m[2];
    if (src.startsWith('qa_images/')) srcs.push(src);
  }
  const htmlRe = /<img\b[^>]*>/gi;
  while ((m = htmlRe.exec(answer)) !== null) {
    if (inFence(m.index)) continue;
    const tag = m[0];
    const sm = tag.match(/\ssrc\s*=\s*(['"])(.*?)\1/i) || tag.match(/\ssrc\s*=\s*([^\s>]+)/i);
    const src = sm ? (sm[2] || sm[1]) : null;
    if (src && src.startsWith('qa_images/')) srcs.push(src);
  }
  return srcs;
}
function injectImageLicenses(data) {
  const licenses = loadLicensesMap();
  const def = licenses['_default'] || { license: 'Apache-2.0' };
  let changed = 0;
  for (const item of data) {
    const srcs = extractImageSrcsForLicenses(item.answer || '');
    if (!srcs.length) {
      if ('image_licenses' in item) { delete item.image_licenses; changed++; }
      continue;
    }
    const map = {};
    for (const src of srcs) {
      const bn = path.basename(src);
      let lic = licenses[src] || licenses[bn] || licenses['qa_images/' + bn];
      if (!lic) lic = def;
      map[src] = lic;
    }
    const newStr = JSON.stringify(map);
    const oldStr = item.image_licenses ? JSON.stringify(item.image_licenses) : null;
    if (newStr !== oldStr) {
      item.image_licenses = map;
      changed++;
    }
  }
  if (changed) console.log(`✅ image_licenses 注入/更新: ${changed} 件`);
  return changed;
}

// ── メイン ────────────────────────────────────────────────
async function main() {
  const raw = fs.readFileSync(JSON_PATH, 'utf-8');
  const data = JSON.parse(raw);

  // 1. search_text 生成 + image_licenses 注入
  if (!SKIP_TEXT) {
    await new Promise((resolve, reject) => {
      // CWD 依存にしない（リポジトリ外から `node scripts/build.js --data …` を
      // 実行しても辞書を見つけられるように __dirname 基準で解決する）
      kuromoji.builder({ dicPath: path.join(__dirname, '..', 'node_modules', 'kuromoji', 'dict') }).build((err, tokenizer) => {
        if (err) { reject(err); return; }
        let changed = 0;
        for (const item of data) {
          const generated = buildSearchText(tokenizer, item);
          if (item.search_text !== generated) { item.search_text = generated; changed++; }
        }
        const licChanged = injectImageLicenses(data);
        if (changed || licChanged) {
          fs.writeFileSync(JSON_PATH, JSON.stringify(data, null, 4), 'utf-8');
          console.log(`✅ search_text 生成完了（${changed} 件更新 / ${data.length} 件中）${licChanged ? ` + image_licenses ${licChanged}件` : ''}`);
        } else {
          console.log(`✅ search_text 生成完了（${changed} 件更新 / ${data.length} 件中） — 変更なし`);
        }
        resolve();
      });
    });
  } else {
    // --embed-only 等で search_text をスキップする場合も image_licenses は注入する
    // (--data モードでは配信ファイルを触らないため version.json 生成はしないが、 licenses 注入は作業コピーに対して行う)
    const licChanged = injectImageLicenses(data);
    if (licChanged) {
      fs.writeFileSync(JSON_PATH, JSON.stringify(data, null, 4), 'utf-8');
      console.log(`✅ image_licenses 注入: ${licChanged} 件`);
    }
  }

  // 2. Embedding 生成
  if (DO_EMBED) {
    await buildEmbeddings(data);
  }

  // 3. version.json 生成（常に最新の on-disk 状態をハッシュ化する）
  //    --data（作業コピー）モードでは配信ファイルを触っていないので生成しない
  if (!CUSTOM_DATA) {
    generateVersionManifest();
  }
}

main().catch(err => { console.error('❌', err); process.exit(1); });