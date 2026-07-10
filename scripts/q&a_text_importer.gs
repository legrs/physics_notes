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
// ============================================================
// Physics Notes JSON ↔ Google Sheets 変換スクリプト
// ============================================================
//
// 変更点（Phase 0）:
//   - 'category'  列を追加（例: "力学", "電磁気学", "熱力学"）
//     ※ 複数割り当て可。1 つのセルに " | " 区切りで並べる（例: "静電気 | 電流"）。
//        配列フィールド（ARRAY_FIELDS）として扱われ、JSON では文字列配列になる。
//   - 'difficulty' 列を追加（例: "基礎", "標準", "発展"）
//
// 変更点（Phase 1）: ID の正規化
//   - 数値 ID は先頭ゼロを付けない正規形で扱う（"00001" → "1"）。
//     規模拡大に伴う桁数固定（5桁）の制約を撤廃する。
//   - ID は数値とは限らない前提で実装する。UUID 等の非数値 ID は
//     そのまま保持する（_normalizeId が数値判定で分岐）。
//   - 番号を意識せず項目を追加できるよう、空の id セルへ UUID を
//     一括付与する assignMissingUUIDs() を用意した（メニューから実行）。
//
// ============================================================

const SHEET_NAME   = 'q_and_a_data';
const SEP          = ' | ';
// category も複数割り当て可能にする（例: "静電気 | 電流"）。
// keywords / synonyms / related と同じく SEP 区切りの配列フィールドとして扱い、
// インポート時はセルへ結合、エクスポート時は配列へ分割する。
const ARRAY_FIELDS = ['questions', 'keywords', 'synonyms', 'related', 'category'];

// ★ search_text はビルドスクリプトが自動生成するが、
//    インポート時に保持・エクスポート時に引き継ぐため COLUMNS に含める
const COLUMNS = [
  'id', 'questions', 'answer', 'description',
  'keywords', 'synonyms',
  'category', 'difficulty',
  'priority', 'related', 'updated_at',
  'search_text',
  '_note',
];

const JSON_URL   = 'https://raw.githubusercontent.com/legrs/physics_notes/refs/heads/master/q_and_a_data.json';
const FILE_NAME  = 'q_and_a_data.json';
const DRIVE_PATH = 'おべんきょ/legrs_physics_notes';

/* ================================================================
   ID 正規化（インポート／エクスポート共通）

   保存形式（ゼロ埋めの有無、数値型かテキスト型か）に依存せず、
   常に「正規形」の文字列 ID を返す。これが ID 操作の唯一の入口で
   あり、検索側（search.html / build.js）も同じ規則で正規化する。

     - 数値 ID         : 先頭ゼロを除去（"00001" → "1", 1.0 → "1"）
     - UUID 等の非数値 : トリムのみ。そのまま保持（将来の UUID 移行用）
     - 空              : "" を返す
================================================================ */
function _normalizeId(val) {
  const s = String(val ?? '').trim();
  if (s === '') return '';
  // 純粋な数値文字列: 先頭ゼロを除去（少なくとも1桁は残す）
  if (/^\d+$/.test(s)) return s.replace(/^0+(?=\d)/, '');
  // スプレッドシートが数値として保持し "1.0" 等になった場合の整数化
  if (/^\d+\.0+$/.test(s)) return s.replace(/\.0+$/, '');
  // UUID 等の非数値 ID はそのまま（将来 UUID を導入してもコード変更不要）
  return s;
}

/* ================================================================
   UUID 生成（将来の UUID 運用向けヘルパー）
================================================================ */
function _generateUUID() {
  return Utilities.getUuid();
}

/* ================================================================
   UUID 判定（8-4-4-4-12 の16進形式）
   既に UUID 化済みの ID を移行対象から除外するために使う。
================================================================ */
function _isUUID(val) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
    .test(String(val ?? '').trim());
}

/* ================================================================
   インポート（URL から取得）
================================================================ */
function importFromURL() {
  const res  = UrlFetchApp.fetch(JSON_URL);
  const data = JSON.parse(res.getContentText());
  _populateSheet(data);
}

/* ================================================================
   インポート（"json_input" シートの A 列から取得）

   エクスポート（exportToSheet）は JSON を改行ごとに 1 行 1 セルへ
   分割して書き出す。JSON が大きくなり 1 セルの文字数上限
   （約 50,000 文字）を超えても扱えるようにするためで、インポートも
   同じ形式に合わせる。
     - A 列の各セル = JSON の 1 行
     - これらを改行で結合して元の JSON 文字列を復元する
   後方互換：A1 セルに JSON 全体を貼り付けた従来形式も、結合後に
   そのままパースできるため引き続き動作する。
================================================================ */
function importFromPaste() {
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  const inputSheet = ss.getSheetByName('json_input');
  if (!inputSheet) {
    SpreadsheetApp.getUi().alert(
      '"json_input" という名前のシートを作り、A 列にJSONを貼り付けてください。'
    );
    return;
  }

  const lastRow = inputSheet.getLastRow();
  if (lastRow < 1) {
    SpreadsheetApp.getUi().alert('"json_input" シートが空です。');
    return;
  }

  // A 列を上から全行取得し、改行で結合して JSON 文字列を復元する
  const lines = inputSheet.getRange(1, 1, lastRow, 1).getValues();
  const text = lines.map(row => String(row[0] ?? '')).join('\n');

  let data;
  try {
    data = JSON.parse(text);
  } catch (e) {
    SpreadsheetApp.getUi().alert(
      'JSON の解析に失敗しました。"json_input" シートの A 列に、' +
      'エクスポート時と同じ形式（1 行 1 セル）で貼り付けてください。\n\n' +
      e.message
    );
    return;
  }
  _populateSheet(data);
}

/* ================================================================
   シートへの書き込み（共通処理）
================================================================ */
function _populateSheet(data) {
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  let sheet = ss.getSheetByName(SHEET_NAME);
  if (!sheet) {
    sheet = ss.insertSheet(SHEET_NAME);
  } else {
    sheet.clearContents();
    sheet.clearFormats();
  }

  sheet.getRange(1, 1, 1, COLUMNS.length).setValues([COLUMNS]);

  const rows = data.map(item =>
    COLUMNS.map(col => {
      let val = item[col];
      // id 列・related 列は正規形（先頭ゼロなし／UUIDはそのまま）で取り込む
      if (col === 'id') {
        val = _normalizeId(val);
      } else if (col === 'related' && Array.isArray(val)) {
        val = val.map(_normalizeId);
      }
      // 先頭が = / + の値は Sheets が数式として解釈・実行してしまう
      // （数式インジェクション）ため、' と同様に ' を前置してテキスト化する。
      // Sheets は先頭の ' を書式マーカーとして取り除くので、エクスポート時の
      // getValues() では元の文字列がそのまま返り、往復しても値は変わらない。
      const _quoteIfNeeded = s => (/^[='+]/.test(s) ? "'" + s : s);
      if (Array.isArray(val)) {
        return _quoteIfNeeded(val.join(SEP));
      }
      return _quoteIfNeeded(String(val ?? ''));
    })
  );

  // id 列・related 列はテキスト書式にして、Sheets による数値化
  // （先頭ゼロの消失や UUID の破損）を防ぐ
  const textCols = ['id', 'related'];
  textCols.forEach(col => {
    const idx = COLUMNS.indexOf(col) + 1;
    if (idx > 0) {
      sheet.getRange(2, idx, Math.max(rows.length, 1), 1).setNumberFormat('@');
    }
  });

  if (rows.length > 0) {
    sheet.getRange(2, 1, rows.length, COLUMNS.length).setValues(rows);
  }

  _applyFormat(sheet);
  SpreadsheetApp.getUi().alert(`✅ ${data.length} 件をインポートしました。`);
}

/* ================================================================
   書式設定
================================================================ */
function _applyFormat(sheet) {
  const lastRow = sheet.getLastRow();

  // ヘッダー
  const header = sheet.getRange(1, 1, 1, COLUMNS.length);
  header.setBackground('#1a73e8').setFontColor('#ffffff').setFontWeight('bold');

  // 列幅（COLUMNS の順番に合わせて設定）
  // id, questions, answer, description, keywords, synonyms, category, difficulty, priority, related, updated_at, search_text, _note
  const widths = [75, 200, 420, 210, 160, 160, 100, 80, 55, 110, 95, 300, 160];
  widths.forEach((w, i) => sheet.setColumnWidth(i + 1, w));

  // answer 列だけ折り返し
  const ansCol = COLUMNS.indexOf('answer') + 1;
  if (ansCol > 0 && lastRow > 1) {
    sheet.getRange(2, ansCol, lastRow - 1, 1)
      .setWrap(true)
      .setVerticalAlignment('top');
  }

  // category 列に薄い背景色をつけて視認しやすくする
  const catCol = COLUMNS.indexOf('category') + 1;
  if (catCol > 0 && lastRow > 1) {
    sheet.getRange(2, catCol, lastRow - 1, 1).setBackground('#e8f0fe');
  }

  // difficulty 列
  const difCol = COLUMNS.indexOf('difficulty') + 1;
  if (difCol > 0 && lastRow > 1) {
    sheet.getRange(2, difCol, lastRow - 1, 1).setBackground('#e6f4ea');
  }

  sheet.setFrozenRows(1);
  sheet.setFrozenColumns(1);
}

/* ================================================================
   Date 型を "YYYY-MM-DD" 文字列に変換
================================================================ */
function _formatDate(date) {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, '0');
  const d = String(date.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

/* ================================================================
   JSON 文字列を生成（エクスポート共通処理）
================================================================ */
function _buildJSON() {
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  const sheet = ss.getSheetByName(SHEET_NAME);
  if (!sheet) {
    SpreadsheetApp.getUi().alert(`"${SHEET_NAME}" シートが見つかりません。`);
    return null;
  }

  const [headers, ...rows] = sheet.getDataRange().getValues();

  const result = rows
    .filter(row => String(row[0]).trim() !== '')
    .map(row => {
      const item = {};
      headers.forEach((col, j) => {
        let val = row[j];
        if (ARRAY_FIELDS.includes(col)) {
          const arr = val ? String(val).split(SEP).map(s => s.trim()).filter(Boolean) : [];
          // related も id と同じ規則で正規化（先頭ゼロなし／UUIDはそのまま）
          item[col] = col === 'related' ? arr.map(_normalizeId) : arr;
        } else if (col === 'priority') {
          item[col] = Number(val) || 0;
        } else if (col === 'id') {
          item[col] = _normalizeId(val);
        } else {
          item[col] = val instanceof Date ? _formatDate(val) : String(val ?? '');
        }
      });
      return item;
    });

  return { json: JSON.stringify(result, null, 4), count: result.length };
}

/* ================================================================
   エクスポート → "json_output" シートに書き出し
================================================================ */
function exportToSheet() {
  const built = _buildJSON();
  if (!built) return;
  const { json, count } = built;

  const ss = SpreadsheetApp.getActiveSpreadsheet();
  let outSheet = ss.getSheetByName('json_output');
  if (!outSheet) outSheet = ss.insertSheet('json_output');
  outSheet.clearContents();

  const lines = json.split('\n');
  outSheet.getRange(1, 1, lines.length, 1).setValues(lines.map(l => [l]));
  outSheet.activate();

  SpreadsheetApp.getUi().alert(
    `✅ ${count} 件をシートに書き出しました。\n\n` +
    `"json_output" シートの A列全体を選択してコピーしてください。`
  );
}

/* ================================================================
   エクスポート → Drive に保存
================================================================ */
function exportToDrive() {
  const built = _buildJSON();
  if (!built) return;
  const { json, count } = built;

  const folder = _getOrCreateFolder(DRIVE_PATH);

  const existing = folder.getFilesByName(FILE_NAME);
  if (existing.hasNext()) {
    existing.next().setContent(json);
  } else {
    folder.createFile(FILE_NAME, json, MimeType.PLAIN_TEXT);
  }

  SpreadsheetApp.getUi().alert(
    `✅ ${count} 件を Drive に保存しました。\n\n` +
    `保存先：マイドライブ / ${DRIVE_PATH} / ${FILE_NAME}`
  );
}

/* ================================================================
   Drive フォルダをパスで取得（なければ作成）
================================================================ */
function _getOrCreateFolder(path_) {
  const parts = path_.split('/').filter(Boolean);
  let folder = DriveApp.getRootFolder();
  for (const part of parts) {
    const sub = folder.getFoldersByName(part);
    folder = sub.hasNext() ? sub.next() : folder.createFolder(part);
  }
  return folder;
}

/* ================================================================
   空の id セルに UUID を一括付与

   「番号を気にせず項目を追加する」運用を可能にするための補助。
   id 列が空の行へ UUID を割り当て、テキスト書式で保存する。
   既存の数値 ID はそのまま残るため、数値運用と UUID 運用を併用できる。
================================================================ */
function assignMissingUUIDs() {
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  const sheet = ss.getSheetByName(SHEET_NAME);
  if (!sheet) {
    SpreadsheetApp.getUi().alert(`"${SHEET_NAME}" シートが見つかりません。`);
    return;
  }

  const idCol = COLUMNS.indexOf('id') + 1;
  const lastRow = sheet.getLastRow();
  if (idCol <= 0 || lastRow < 2) {
    SpreadsheetApp.getUi().alert('対象となる行がありません。');
    return;
  }

  const range = sheet.getRange(2, idCol, lastRow - 1, 1);
  const values = range.getValues();
  let filled = 0;
  for (let i = 0; i < values.length; i++) {
    if (String(values[i][0]).trim() === '') {
      values[i][0] = _generateUUID();
      filled++;
    }
  }

  range.setNumberFormat('@').setValues(values);
  SpreadsheetApp.getUi().alert(`✅ ${filled} 件の空 ID に UUID を割り当てました。`);
}

/* ================================================================
   既存の数値 ID を UUID へ一括移行

   - 数値 ID の各行へ新しい UUID を割り当てる。
   - related 列の参照も同じ対応表で UUID へ置換する。
   - シートは ID 順に並んでいるとは限らないため、まず全行を読んで
     「旧ID(正規化) → 新UUID」の対応表を作り、その後で id 列・related 列を
     一括置換する（位置に依存しない）。
   - 既に UUID の ID はスキップ（再実行しても二重移行しない）。
   - 対応表に無い related 参照（既存UUID・他所への参照・欠番）は
     正規化した値のまま温存する。
   - 破壊的操作のため実行前に確認ダイアログを出す。

   ※ ID が変わると embeddings.json のキーも変わるため、移行後は
     必ず Embedding を再生成すること（node scripts/build.js --embed）。
================================================================ */
function migrateToUUID() {
  const ui = SpreadsheetApp.getUi();
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  const sheet = ss.getSheetByName(SHEET_NAME);
  if (!sheet) {
    ui.alert(`"${SHEET_NAME}" シートが見つかりません。`);
    return;
  }

  const idCol  = COLUMNS.indexOf('id') + 1;
  const relCol = COLUMNS.indexOf('related') + 1;
  const lastRow = sheet.getLastRow();
  if (idCol <= 0 || lastRow < 2) {
    ui.alert('移行対象の行がありません。');
    return;
  }

  const resp = ui.alert(
    'UUID への移行',
    '既存の数値 ID をすべて UUID へ置き換え、related 参照も連動して書き換えます。\n' +
    'この操作は元に戻せません。続行しますか？\n\n' +
    '（移行後は Embedding の再生成が必要です）',
    ui.ButtonSet.YES_NO
  );
  if (resp !== ui.Button.YES) return;

  const n = lastRow - 1;

  // ── 1) 全 id を読み、旧ID(正規化) → 新UUID の対応表を作る ──────────
  const idValues = sheet.getRange(2, idCol, n, 1).getValues();
  const idMap = {};            // 正規化済み旧ID -> 新UUID
  const newIds = new Array(n);  // 書き戻し用（[[uuid], ...]）
  let migrated = 0, skipped = 0;

  for (let i = 0; i < n; i++) {
    const norm = _normalizeId(idValues[i][0]);
    if (norm === '') {            // 空行はそのまま
      newIds[i] = [''];
      continue;
    }
    if (_isUUID(norm)) {           // 既に UUID ならスキップ
      newIds[i] = [norm];
      skipped++;
      continue;
    }
    // 同じ旧IDは同じ UUID に対応させる（related 置換の一貫性のため）
    if (!idMap[norm]) idMap[norm] = _generateUUID();
    newIds[i] = [idMap[norm]];
    migrated++;
  }

  // ── 2) related 列を対応表で置換 ──────────────────────────────────
  let newRels = null;
  if (relCol > 0) {
    const relValues = sheet.getRange(2, relCol, n, 1).getValues();
    newRels = relValues.map(([cell]) => {
      const s = String(cell ?? '');
      if (s.trim() === '') return [''];
      const replaced = s.split(SEP)
        .map(p => p.trim())
        .filter(Boolean)
        .map(p => {
          const norm = _normalizeId(p);
          return idMap[norm] || norm; // 対応表にあれば UUID、無ければ正規化値を温存
        });
      return [replaced.join(SEP)];
    });
  }

  // ── 3) 書き戻し（id・related ともテキスト書式で保存）─────────────
  sheet.getRange(2, idCol, n, 1).setNumberFormat('@').setValues(newIds);
  if (newRels) {
    sheet.getRange(2, relCol, n, 1).setNumberFormat('@').setValues(newRels);
  }

  ui.alert(
    `✅ ${migrated} 件の数値 ID を UUID へ移行しました` +
    (skipped ? `（既存 UUID ${skipped} 件はスキップ）` : '') + '。\n' +
    'related 参照も置換済みです。\n\n' +
    '⚠️ Embedding の再生成を忘れずに：node scripts/build.js --embed'
  );
}

/* ================================================================
   メニュー
================================================================ */
function onOpen() {
  SpreadsheetApp.getUi()
    .createMenu('📋 Physics Notes')
    .addItem('🔽 インポート（URL から）',         'importFromURL')
    .addItem('🔽 インポート（貼り付けから）',      'importFromPaste')
    .addSeparator()
    .addItem('🔼 エクスポート → シートに書き出し', 'exportToSheet')
    .addItem('🔼 エクスポート → Drive に保存',     'exportToDrive')
    .addSeparator()
    .addItem('🆔 空の ID に UUID を割り当て',      'assignMissingUUIDs')
    .addItem('🔄 数値 ID を UUID へ一括移行',       'migrateToUUID')
    .addToUi();
}