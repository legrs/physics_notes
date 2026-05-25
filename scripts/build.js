#!/usr/bin/env node
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
const kuromoji = require('kuromoji');

const JSON_PATH = path.join(__dirname, '..', 'q_and_a_data.json');
const EMBEDDINGS_PATH = path.join(__dirname, '..', 'embeddings.json');

const DO_EMBED = process.argv.includes('--embed') || process.argv.includes('--embed-only');
const SKIP_TEXT = process.argv.includes('--embed-only');

// ── モデル定義 ───────────────────────────────────────────────
const MODELS = {
  small: { id: 'Xenova/multilingual-e5-small', dim: 384 },
  large: { id: 'Xenova/multilingual-e5-large', dim: 1024 },
};

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
function buildSearchText(tokenizer, item) {
  const fields = [
    ...(item.questions || []),
    item.description || '',
    ...(item.keywords || []),
    ...(item.synonyms || []),
    item.category || '',
  ];
  const cleaned = fields.map(s => stripLatex(String(s))).filter(Boolean).join(' ');
  const kata = getReading(tokenizer, cleaned);
  const hira = toHiragana(kata);
  const parts = new Set([cleaned]);
  if (kata !== cleaned) parts.add(kata);
  if (hira !== cleaned && hira !== kata) parts.add(hira);
  return [...parts].join(' ').replace(/\s+/g, ' ').trim();
}

// ── Embedding 生成 ─────────────────────────────────────────
// e5 モデルは文書に "passage: " プレフィックスが必要
async function buildEmbeddings(data) {
  const { pipeline } = await import('@xenova/transformers');

  const existing = fs.existsSync(EMBEDDINGS_PATH)
    ? JSON.parse(fs.readFileSync(EMBEDDINGS_PATH, 'utf-8'))
    : {};

  for (const [key, model] of Object.entries(MODELS)) {
    console.log(`\n📐 ${key}（${model.id}）のEmbeddingを生成中...`);
    const extractor = await pipeline('feature-extraction', model.id, { quantized: true });

    if (!existing[key]) existing[key] = {};
    let updated = 0;

    for (let i = 0; i < data.length; i++) {
      const item = data[i];
      // 既に生成済みでdim数が合っていればスキップ
      if (existing[key][item.id]?.length === model.dim) continue;

      // passage プレフィックスを付けた文書テキスト
      const text = `passage: ${item.questions[0]} ${item.description}`;
      const out = await extractor(text, { pooling: 'mean', normalize: true });
      existing[key][item.id] = Array.from(out.data);
      updated++;

      if ((i + 1) % 10 === 0 || i === data.length - 1) {
        process.stdout.write(`  ${i + 1}/${data.length} 件完了\r`);
      }
    }
    console.log(`  ✅ ${key}: ${updated} 件更新`);
  }

  fs.writeFileSync(EMBEDDINGS_PATH, JSON.stringify(existing), 'utf-8');
  console.log(`\n💾 embeddings.json を保存しました`);
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
}

main().catch(err => { console.error('❌', err); process.exit(1); });
