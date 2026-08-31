# qa_images

QAコーパス由来の写真専用フォルダ。`q_and_a_data.json` の `answer` から `![](qa_images/<uuid>.<ext>)` で参照する。

## 運用

- 手元では `photo_001.jpg` のような雑な名前で置き、`answer` に `![](qa_images/photo_001.jpg)` と書いて push するだけでよい。CI (`build.yml`) が `qa_images/<uuid>.jpg` にリネームし、`q_and_a_data.json` 内参照も `licenses.json` のキーも追従して `[skip ci]` で再pushする。
- 拡張子は小文字に正規化される (`.JPG` → `.jpg`)。

## 推奨

- WebP 推奨 (サイズ削減) だが強制しない。対応拡張子: `jpg/jpeg/png/webp/svg/gif`
- 5MB超は警告のみ (reject しない)。CIで `0<=x<3MB / 3<=x<5MB / 5MB<=` の統計と平均を出力。

## ライセンス

- デフォルトはリポジトリ `LICENSE` の Apache-2.0。
- 個別例外のみ `licenses.json` に上書きを記載 (未記載はデフォ)。例:

```json
{
  "_default": {"license": "Apache-2.0"},
  "3f9a8c1e-1a2b-4c3d-9e8f-a1b2c3d4e5f6.jpg": {"license": "CC BY-SA 4.0", "attribution": "出典: 教科書p42", "url": "https://example.com"}
}
```

- `answer` 内で `*出典: ...*` と手書きしても可。`build.js` が `licenses.json` を読み、各レコードに `image_licenses` として注入する (表示側はそれを `<figcaption>` / Detailに使う)。

## ローカルでの正規化

```sh
npm run normalize:images        # リネーム + JSON書き換え
npm run normalize:images:check  # dry-run (CI検証用)
```
