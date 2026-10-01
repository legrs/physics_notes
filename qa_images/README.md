# qa_images

QAコーパスの写真・動画のフォルダ。`q_and_a_data.json` の `answer` から `![](qa_images/<uuid>.<ext>)` で参照する。

動画も同じ記法で、拡張子で判別される（`![振り子の実験](qa_images/<uuid>.mp4)` → `<video controls>`）。HTML の `<video src>` / `<source src>` / `<video poster>` も可。

## 運用

- 手元では `photo_001.jpg` のような雑な名前で置き、`answer` に `![](qa_images/photo_001.jpg)` と書いて push するだけでよい。CI (`build.yml`) が `qa_images/<uuid>.jpg` にリネームし、`q_and_a_data.json` 内参照も `licenses.json` のキーも追従して `[skip ci]` で再pushする。
- 拡張子は小文字に正規化される (`.JPG` → `.jpg`)。

## 推奨

- WebP 推奨 (サイズ削減) だが強制しない。対応拡張子: `jpg/jpeg/png/webp/svg/gif`
- 透過 PNG / SVG は、透明部分が表示場所の背景色（ライト=白 / ダーク=黒系）になる。ダークモードでは黒い線の図が見えにくくなるので、白背景が必要な図は背景を塗った画像にしておく。SVG は `viewBox` だけでも表示されるが、`width`/`height` を付けると表示サイズが安定する
- **HEIC / HEIF（iPhone の写真）もそのまま置いてよい**。Chrome/Firefox では表示できないため、`normalize-images.js`（CI）が `<uuid>.jpg` に変換し、元ファイルは削除、`answer` の参照と `licenses.json` のキーも追従する。変換時に位置情報などの EXIF は削除される
- 動画の対応拡張子: `mp4/m4v/webm/ogv/mov`。**H.264 の `.mp4` を推奨**（全ブラウザで再生可）。iPhone の `.mov`（HEVC）は Chrome/Firefox で再生できないことがあるので変換推奨。
- 動画はリポジトリにそのまま入るため、短く圧縮して **20MB 未満** を目安に（超えると `normalize-images.js` と CI が警告。GitHub は 100MB 超のファイルを拒否）。長い動画は YouTube 等に置いてリンクする方がよい。
- physq（CLI）では Detail の 🎬 行から既定のブラウザ/プレイヤーで開く（`search --plain` では `video\t<ラベル>\t<URL>` 行）。
- 5MB超は警告のみ (reject しない)。CIで `0<=x<3MB / 3<=x<5MB / 5MB<=` の統計と平均を出力（動画は 20MB 以上で警告）。

## ライセンス

**`media_editor.html`（Media Editor）で一覧・確認・編集できる。** GitHub から最新の `qa_images/`・`licenses.json`・`q_and_a_data.json` を読み、ファイルごとにプレビュー（透過の確認用に背景を市松/白/黒で切替）・使われている回答・実際に表示されるキャプションを確認しながらライセンスを編集し、最後に生成された `licenses.json` をコピー（または保存）して置き換え・push する。未使用・参照切れ・HEIC 変換待ち・質問ごとに値が食い違うファイルなどは「要確認」フィルタで絞り込める。スマホでは 一覧/表示/ライセンス/コード のタブ表示。

優先順位（`scripts/build.js` の `injectImageLicenses`）: **`licenses.json` のファイル個別指定 ＞ `q_and_a_data.json` の質問ごとの `image_licenses`（qa_editor の「画像ライセンス」欄）＞ `licenses.json` の `_default`**。個別指定があるファイルでは qa_editor で入れた値は使われない。

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
