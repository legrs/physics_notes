# 松本深志高校　3年物理ノート

ノート　Note：\
<https://legrs.github.io/physics_notes/electromagnetism.pdf>

先生とのQ&Aデータベース検索エンジン　Q&A database search engine for faculty：\
<https://legrs.github.io/physics_notes/>

Physics Notes CLI（ターミナルから検索できるツール `physq`）：\
[physq/README.md](physq/README.md) | [最新リリース / Latest release](https://github.com/legrs/physics_notes/releases/)

## Project Structure

```
physics_notes/
├── fast/                              # 軽量モード（mode=fast）入口。本編へリダイレクトするだけ
│   ├── index.html                     #  → ../index.html?mode=fast
│   └── search.html                    #  → ../search.html?mode=fast へ q パラメータも引き継ぐ
├── img/
│   └── ai-icon.svg                    # サイトのアイコン（AI ロゴ）
├── physq/                             # 同じコーパスをターミナルで検索する Rust CLI（詳細は physq/README.md）
│   ├── scripts/
│   │   └── parity_check.js            # search.html の実スコア関数と Rust 版のランキング一致を検証
│   ├── src/
│   │   ├── bm25/                      # BM25 索引（kuromoji/lindera による語彙検索）
│   │   ├── data/                      # データ取得とキャッシュ（version.json のハッシュ照合）
│   │   ├── query/                     # クエリのトークナイズと同義語展開
│   │   ├── rank/                      # RRF 融合（BM25 と意味検索のスコア統合）
│   │   ├── semantic/                  # e5 意味検索（ONNX でクエリ埋め込みのみ計算）
│   │   ├── tui/                       # 対話型 TUI（command.rs: スラッシュコマンド / vim.rs: Vim 風キー）
│   │   ├── cli.rs                     # clap の CLI 定義（TUI / search / cache / update / eval）
│   │   ├── config.rs                  # 設定：データホストURL・キャッシュ配置・モデル選択
│   │   ├── engine.rs                  # UI 非依存の検索エンジンのファサード
│   │   ├── eval.rs                    # `physq eval`：自己改善ループ用のランキング評価
│   │   ├── model.rs                   # コーパスレコード定義と id 正規化（build.js と同期）
│   │   ├── real_data_tests.rs         # 実データを使ったランキングの parity テスト
│   │   ├── spinner.rs                 # ローディング表示（braille スピナー）
│   │   ├── update.rs                  # GitHub Releases からの自己更新
│   │   ├── lib.rs / main.rs           # crate のエントリポイント
│   │   └── (その他 tui/ 配下に command.rs・vim.rs 等)
│   ├── Cargo.toml / Cargo.lock        # Rust の依存マニフェスト
│   └── README.md                      # physq の仕様兼ユーザーガイド
├── scripts/                           # データパイプラインと自動改善ツール
│   ├── build.js                       # search_text と embeddings.json を自動生成するビルド
│   ├── model_bakeoff.py               # 自己改善用 LLM を lms ls で比較選定するスクリプト
│   ├── q&a_text_importer.gs           # Google Sheets ↔ q_and_a_data.json の変換（Apps Script）
│   └── self_improve.py                # LLM で検索データを自動改善する夜間ループ
├── debug_search.html                  # 検索ページの検証用コピー（モード比較・デバッグ専用）
├── dirlens.txt                        # リポジトリ解析ツール（dirlens）の出力（CI 生成物）
├── electromagnetism.typ / .pdf        # Typst で書かれた物理ノート（PDF を README から公開）
├── embeddings.json                    # 生成済み e5 埋め込み（search.html と physq が共有）
├── index.html                         # ランディングページ（検索ページへ誘導）
├── LICENSE / NOTICE                   # Apache 2.0 ライセンス
├── package.json / package-lock.json   # ビルド用 JS 依存（kuromoji・transformers.js）
├── phy.typ                            # 物理ノート用の共通定義（回路記号などの cetz 描画）
├── q_and_a.txt                        # 先生の質問の模範解答テキスト集
├── q_and_a_data.json                  # メインの Q&A コーパス（Google Sheets から書き出し・264 件）
├── q_and_a_data_handcrafted.json      # 手作業で整えたデータセット案（パイプライン未接続）
├── qa_images/                         # QA写真専用（`![](qa_images/<uuid>.jpg)` で answer から参照、CIがUUID正規化）
│   ├── licenses.json                  # 個別ライセンス上書き（未記載は Apache-2.0）
│   └── <uuid>.jpg                     # 実体はUUIDファイル名のみ（手元では雑な名前で置いてpushすればCIが直す）
├── qa_editor.html                     # Q&A データの編集ツール（<title>Q&A Editor</title>）
├── search.html                        # 検索 UI 本体（BM25 + e5 + RRF のハイブリッド検索）
├── template.typ / .pdf                # Typst ノートの共通テンプレート（ページ設定・数式スタイル）
└── version.json                       # 生成物のハッシュ・埋め込みモデル情報（キャッシュ整合検証用。`qa_images` のハッシュも含む）
```

## テスト / Tests

push のたびに GitHub Actions（`.github/workflows/test.yml`）が HTML・データ・physq を
並列で検証します（`build.yml` とは独立。生成物のコミットはしない）。

```sh
npm install      # 初回のみ（テストに kuromoji が必要）
npm test         # 全部まとめてローカル実行
npm run test:html   # HTML 構文 + search.html の実スコア関数ストレステスト
npm run test:data   # JSON 生成物の整合性 + web↔physq parity
npm run test:physq  # physq の実バイナリで検索ストレステスト（実データ、モデルDLなし）
```

- **HTML**: インライン `<script>` の構文チェック、CDN の SRI 固定、必須 ID、`search.html` の
  実スコア関数（BM25/e5/RRF）を実データ＋極端な入力（空・巨大・XSS・不正ユニコード等）で大量実行。
- **データ**: スキーマ・`related` 整合性・`embeddings.json` の次元/網羅性・`version.json` の
  hash 照合・`search_text` が再生成と一致すること（決定性チェック）。
- **physq**: 各プラットフォーム（matrix。`.github/workflows/test.yml` の include を編集して
  追加・削除）でビルド → `cargo test`/`clippy`/`fmt` → 実バイナリで `eval` ストレステスト
  （`--model none`＝BM25のみでモデルDLなし）。

## 検索の自己改善ループ（scripts/self_improve.py）

LM Studio のローカル LLM と `physq eval` を使って、検索データセット
（`keywords` / `synonyms` / `questions`）を自動改善するループです。
LLM が「同じ意味の言い換え質問」を生成して検索の弱点を探し、1位を取れなかった
レコードに語を追加 → 実際に順位が改善した編集だけを採用、を繰り返します。
編集は作業コピー（`self_improve_work/`、git 管理外）に対して行われ、
`--apply-only` で確認してから本番へ反映します。

```sh
# 事前準備: LM Studio を起動しサーバを有効化、physq を physq/ でビルド、npm install 済みであること
python3 scripts/self_improve.py --model <LM StudioのモデルID> --records 5 --cycles 1  # 動作確認
python3 scripts/self_improve.py --model <モデルID> --hours 8 --tune-weights          # 一晩コース
# 翌朝: self_improve_work/report.md を確認して
python3 scripts/self_improve.py --apply-only   # 本番 q_and_a_data.json へ反映
git diff q_and_a_data.json                     # 差分を確認してコミット
```

中断（Ctrl-C）してもチェックポイントから再開できます。詳細は
`python3 scripts/self_improve.py --help` と
[physq/README.md の Ranking evaluation 節](physq/README.md#ranking-evaluation-physq-eval) を参照。

## データベースの書き方

```json
[
    {
        "id": "00001",
        "questions": [
            "What is AI?",
            "Explain artificial intelligence"
        ],
        "answer": "AI is ...",
        "description": "Basic explanation of AI",
        "keywords": [
            "ai",
            "technology"
        ],
        "synonyms": [
            "artificial intelligence"
        ],
        "priority": 2,
        "related": [
            "00002"
        ],
        "updated_at": "2026-04-28",
        "search_text": "what is ai artificial intelligence explain technology",
        "_note": "ここにメモを入力"
    }
]
```

- answerは、マークダウン形式の記述や、LaTeXの使用が可能です。

```json
"answer": "### 力学とは\n物体の**運動**を扱う物理学の分野です。\n\nニュートンの第二法則：\n$$F = ma$$\n\nここで $F$ は力、$m$ は質量、$a$ は加速度です。"
```

## `search_text` の書き方ガイド

このプロジェクトでは、検索精度を高めるために `search_text` フィールドを使用します。  
基本はシンプルですが、いくつかのルールを守ることで精度が大きく向上します。

---

### 基本ルール

#### 1. 半角スペース区切りで記述する

```json
"search_text": "物理とは何か 物理 世界 physics"
```

- 単語やフレーズを**半角スペースで区切る**だけでOK
- JavaScriptで簡単に検索処理ができる

---

#### 2. 英語は小文字で統一する

```json
"physics"
```

- `Physics` や `PHYSICS` などの揺れを防ぐ
- 検索時の一致率を上げる

---

#### 3. 記号は入れない

```diff
- 物理とは何か？
+ 物理とは何か
```

- `？` や `!` などは検索の邪魔になるため除外する

---

### 日本語の扱い（重要）

日本語は単語の区切りがないため、そのままだと検索精度が下がることがあります。

#### NG例

```json
"search_text": "物理とは何か"
```

#### 推奨例

```json
"search_text": "物理とは何か 物理 とは 何か 世界 physics"
```

- フレーズに加えて**単語単位でも分解して追加**する
- 検索ヒット率が大幅に向上する

---

### 推奨構成

`search_text` には以下の要素をすべて含めると効果的です：

- `questions` の内容
- `keywords`
- `synonyms`

#### 例

```json
"search_text": "物理とは何か 物理 とは 何か 世界 physics"
```

---

### 検索処理の例（AND検索）

```js
query.split(" ").every(word => item.search_text.includes(word))
```

- 入力されたすべての単語を含むデータのみヒット
- シンプルかつ実用的

---

### まとめ

- 半角スペース区切りでOK
- 英語は小文字に統一
- 記号は入れない
- 日本語は単語単位に分解する
- 関連語（keywords / synonyms）も含める

---

このルールに従うことで、シンプルな実装でも十分に実用的な検索が可能になります。

---

## QA画像（qa_images/）

`answer` は Markdown の画像記法 `![alt](qa_images/<uuid>.jpg)` で写真を埋め込めます。1レコードに複数枚可（縦積み）。横並びが必要な場合のみ `<div class="img-row">![](qa_images/a.jpg) ![](qa_images/b.jpg)</div>` を使えます。`alt` は検索対象（BM25）になりますが `src`（URL）は語彙にしません。

* **手元では雑な名前でOK** — `photo_001.JPG` のような名前で `qa_images/` に置き、`answer` に `![](qa_images/photo_001.JPG)` と書いて push するだけで、CI（`.github/workflows/build.yml`）が `qa_images/<uuid>.jpg` にリネームし、`q_and_a_data.json` 内参照も `qa_images/licenses.json` のキーも追従して `[skip ci]` で再pushします。
* **ライセンス** — デフォルトは `LICENSE` の Apache-2.0。例外のみ `qa_images/licenses.json` に記載します。`scripts/build.js` が `licenses.json` を読み、各レコードに `image_licenses` として埋め込むため、Web は `<figcaption>`、physq は Detail で自動表示されます。
* **ローカルで正規化** — `npm run normalize:images`（実行） / `npm run normalize:images:check`（dry-run）。
* **サイズ** — `webp` 推奨ですが強制しません。CI は `<3MB / 3-5MB / ≥5MB` の統計を出力し、5MB超は警告のみです。詳細は `qa_images/README.md` を参照。
