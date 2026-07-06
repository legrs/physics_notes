# 松本深志高校　3年物理ノート

ノート　Note：\
<https://legrs.github.io/physics_notes/electromagnetism.pdf>

先生とのQ&Aデータベース検索エンジン　Q&A database search engine for faculty：\
<https://legrs.github.io/physics_notes/>

Physics Notes CLI（ターミナルから検索できるツール `physq`）：\
[physq/README.md](physq/README.md) | [最新リリース / Latest release](https://github.com/legrs/physics_notes/releases/)

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
