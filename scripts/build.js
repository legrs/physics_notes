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
// =============================================================

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const kuromoji = require('kuromoji');

const JSON_PATH = path.join(__dirname, '..', 'q_and_a_data.json');
const EMBEDDINGS_PATH = path.join(__dirname, '..', 'embeddings.json');
const VERSION_PATH = path.join(__dirname, '..', 'version.json');

const DO_EMBED = process.argv.includes('--embed') || process.argv.includes('--embed-only');
const SKIP_TEXT = process.argv.includes('--embed-only');

// ── モデル定義 ───────────────────────────────────────────────
const MODELS = {
  small: { id: 'Xenova/multilingual-e5-small' },
  large: { id: 'Xenova/multilingual-e5-large' },
};

// ── version.json スキーマ定数 ─────────────────────────────
// physq (CLI) の CLAUDE.md §3/§5/§8 で決め打ちされている値と揃える。
// tokenizer が変わったら physq 側の BM25 インデックスキャッシュが再構築される。
const VERSION_SCHEMA = 3;
const TOKENIZER_TAG = 'lindera-ipadic';
const EMBEDDING_MODEL_TAG = 'multilingual-e5-small';

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
  const cleaned = fields.map(s => stripLatex(String(s))).filter(Boolean).join(' ');

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

function generateVersionManifest() {
  const manifest = {
    generated_at: new Date().toISOString(),
    schema_version: VERSION_SCHEMA,
    tokenizer: TOKENIZER_TAG,
    embedding_model: EMBEDDING_MODEL_TAG,
    files: {
      'q_and_a_data.json': fileManifest(JSON_PATH),
      'embeddings.json': fileManifest(EMBEDDINGS_PATH),
    },
  };
  fs.writeFileSync(VERSION_PATH, JSON.stringify(manifest, null, 2) + '\n', 'utf-8');
  console.log('✅ version.json 生成完了');
}

// ── メイン ────────────────────────────────────────────────
async function main() {
  const raw = fs.readFileSync(JSON_PATH, 'utf-8');
  const data = JSON.parse(raw);

  // 1. search_text 生成
  if (!SKIP_TEXT) {
    await new Promise((resolve, reject) => {
      kuromoji.builder({ dicPath: 'node_modules/kuromoji/dict' }).build((err, tokenizer) => {
        if (err) { reject(err); return; }
        let changed = 0;
        for (const item of data) {
          const generated = buildSearchText(tokenizer, item);
          if (item.search_text !== generated) { item.search_text = generated; changed++; }
        }
        fs.writeFileSync(JSON_PATH, JSON.stringify(data, null, 4), 'utf-8');
        console.log(`✅ search_text 生成完了（${changed} 件更新 / ${data.length} 件中）`);
        resolve();
      });
    });
  }

  // 2. Embedding 生成
  if (DO_EMBED) {
    await buildEmbeddings(data);
  }

  // 3. version.json 生成（常に最新の on-disk 状態をハッシュ化する）
  generateVersionManifest();
}

main().catch(err => { console.error('❌', err); process.exit(1); });