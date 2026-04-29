松本深志高校　3年物理ノート

先生とのQ&Aデータベース↓\
<https://legrs.github.io/physics_notes/>

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
