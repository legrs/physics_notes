#!/usr/bin/env node
// =============================================================
// scripts/build.js
// q_and_a_data.json の search_text を自動生成する
// 使い方: node scripts/build.js
// =============================================================

const fs = require('fs');
const path = require('path');
const kuromoji = require('kuromoji');

const JSON_PATH = path.join(__dirname, '..', 'q_and_a_data.json');

// ── LaTeX 数式を除去 ─────────────────────────────────────────
function stripLatex(str) {
    return str
        .replace(/\$\$[\s\S]*?\$\$/g, ' ')     // $$...$$ ブロック数式
        .replace(/\$[^$\n]+?\$/g, ' ')          // $...$ インライン数式
        .replace(/\\[a-zA-Z]+\{[^}]*\}/g, ' ') // \cmd{...}
        .replace(/\\[a-zA-Z]+/g, ' ')           // \cmd
        .replace(/[{}^_]/g, ' ')
        .replace(/\s+/g, ' ')
        .trim();
}

// ── カタカナ → ひらがな ───────────────────────────────────────
function toHiragana(str) {
    return str.replace(/[\u30a1-\u30f6]/g, ch =>
        String.fromCharCode(ch.charCodeAt(0) - 0x60)
    );
}

// ── kuromojiで読みを取得（カタカナ文字列を返す）──────────────
function getReading(tokenizer, text) {
    return tokenizer.tokenize(text)
        .map(t => t.reading || t.surface_form) // 読みがない記号等はそのまま
        .join('');
}

// ── search_text を生成 ────────────────────────────────────────
function buildSearchText(tokenizer, item) {
    const fields = [
        ...(item.questions || []),
        item.description || '',
        ...(item.keywords || []),
        ...(item.synonyms || []),
        item.category || '',
    ];

    const cleaned = fields
        .map(s => stripLatex(String(s)))
        .filter(Boolean)
        .join(' ');

    // kuromojiで漢字→カタカナ読み→ひらがなに変換
    const readingKata = getReading(tokenizer, cleaned);
    const readingHira = toHiragana(readingKata);

    // 重複を避けて追加
    const parts = new Set([cleaned]);
    if (readingKata !== cleaned) parts.add(readingKata);
    if (readingHira !== cleaned && readingHira !== readingKata) parts.add(readingHira);

    return [...parts].join(' ').replace(/\s+/g, ' ').trim();
}

// ── メイン（kuromojiは非同期初期化が必要）───────────────────
kuromoji.builder({ dicPath: 'node_modules/kuromoji/dict' }).build((err, tokenizer) => {
    if (err) {
        console.error('❌ kuromoji の初期化に失敗しました:', err);
        process.exit(1);
    }

    const raw = fs.readFileSync(JSON_PATH, 'utf-8');
    const data = JSON.parse(raw);

    let changed = 0;
    for (const item of data) {
        const generated = buildSearchText(tokenizer, item);
        if (item.search_text !== generated) {
            item.search_text = generated;
            changed++;
        }
    }

    fs.writeFileSync(JSON_PATH, JSON.stringify(data, null, 4), 'utf-8');
    console.log(`✅ search_text 生成完了（${changed} 件更新 / ${data.length} 件中）`);
});