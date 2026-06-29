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
//   - 'difficulty' 列を追加（例: "基礎", "標準", "発展"）
//
// ============================================================

const SHEET_NAME   = 'q_and_a_data';
const SEP          = ' | ';
const ARRAY_FIELDS = ['questions', 'keywords', 'synonyms', 'related'];

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
   インポート（URL から取得）
================================================================ */
function importFromURL() {
  const res  = UrlFetchApp.fetch(JSON_URL);
  const data = JSON.parse(res.getContentText());
  _populateSheet(data);
}

/* ================================================================
   インポート（"json_input" シートの A1 セルから取得）
================================================================ */
function importFromPaste() {
  const ss = SpreadsheetApp.getActiveSpreadsheet();
  const inputSheet = ss.getSheetByName('json_input');
  if (!inputSheet) {
    SpreadsheetApp.getUi().alert(
      '"json_input" という名前のシートを作り、A1 セルにJSONを貼り付けてください。'
    );
    return;
  }
  const data = JSON.parse(inputSheet.getRange('A1').getValue());
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
      const val = item[col];
      if (Array.isArray(val)) {
        const joined = val.join(SEP);
        return joined.startsWith("'") ? "'" + joined : joined;
      }
      const str = String(val ?? '');
      return str.startsWith("'") ? "'" + str : str;
    })
  );

  const relatedColIdx = COLUMNS.indexOf('related') + 1;
  if (relatedColIdx > 0) {
    sheet.getRange(2, relatedColIdx, Math.max(rows.length, 1), 1).setNumberFormat('@');
  }

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
          item[col] = col === 'related'
            ? arr.map(s => /^\d+$/.test(s) ? s.padStart(5, '0') : s)
            : arr;
        } else if (col === 'priority') {
          item[col] = Number(val) || 0;
        } else if (col === 'id') {
          item[col] = String(Math.floor(Number(val))).padStart(5, '0');
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
    .addToUi();
}