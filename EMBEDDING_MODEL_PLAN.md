# Embedding モデル移行計画（案）

現行の `multilingual-e5-small`（384d）+ `multilingual-e5-large`（1024d）を、
日本語・英語により強いモデルへ置き換えるための調査と手順の計画。
**これは計画のみで、実装はまだ行わない。**

作成: 2026-07-07 / 前提バージョン: physq v0.1.5-rc3、コーパス 220 レコード

---

## 1. 前提と制約（モデル選定より先に決まっていること）

このプロジェクトの embedding は **3 か所で同じモデル**を参照している:

| 場所 | 役割 | ランタイム |
|---|---|---|
| `scripts/build.js` | コーパス側（`passage:` 等）を事前計算 → `embeddings.json` | Node + `@xenova/transformers`（ONNX） |
| `search.html` | クエリ側をブラウザ内で計算 | transformers.js（CDN、ONNX、ユーザーが毎回 DL） |
| `physq` | クエリ側をローカルで計算 | Rust `fastembed` / `ort`（ONNX） |

したがって候補モデルは以下を **すべて** 満たす必要がある:

1. **ONNX 変換済み（または変換容易）** で transformers.js と ort の両方で動くこと
2. **ブラウザ配布に耐えるサイズ**（量子化後 ~100–150 MB 程度まで。現行 e5-small は
   quantized で約 100 MB）。CLI 側はもう少し大きくても許容できる
3. ライセンスが静的サイト + バイナリ配布と両立すること
4. `embeddings.json` の肥大が許容範囲（サイズ ∝ 次元数。現行 384+1024 次元で 6 MB）

**Web と CLI で別モデルにする案について**: `embeddings.json` はすでに
`{"small": {...}, "large": {...}}` とモデル別キーなので技術的には可能だが、
事前計算データが 2 系統になり、build.js・CI・検証（parity）・自己改善ループの
評価軸もすべて 2 系統になる。**共有 1 モデル（+移行期間中のみ旧モデル併存）を推奨。**

## 2. 候補モデルの整理

| モデル | パラメタ | 次元 | JP/EN | ONNX/transformers.js | ライセンス | 所見 |
|---|---|---|---|---|---|---|
| **EmbeddingGemma-300m** | 308M | 768（MRL で 512/256/128 に切詰可） | 多言語 100+、JP/EN 良 | 公式 ONNX あり、transformers.js 対応（v3.7+） | Gemma 利用規約 | **本命**。オンデバイス前提設計でブラウザ適性が最良。MRL で embeddings.json も小さくできる |
| **Qwen3-Embedding-0.6B** | 595M | 1024（MRL 対応） | 多言語 MTEB 上位、JP 強 | 公式 ONNX あり、transformers.js 例あり | Apache-2.0 | 品質は候補中最上位級だがブラウザには重い（q8 で ~600 MB 級）。**CLI 専用ならあり** |
| **BGE-M3** | 568M | 1024 | 多言語、JP まずまず | ONNX あり、実績豊富 | MIT | 実績は十分だが 2024 年世代。サイズも重め。Qwen3/Gemma に対する優位が薄い |
| **Ruri v3-310m** | 315M | 768 | **JP 特化で最強クラス**、EN は弱め | ModernBERT-ja ベース。transformers.js は v3.4+ の ModernBERT 対応で動く見込み（要検証） | Apache-2.0 | 質問の大半が日本語なら有力。英語質問・英語キーワード検索が弱くなるのが減点 |
| Sarashina-Embedding-v1-1.2B | 1.2B | 1792 | JP 最強クラス | ONNX 要自前変換 | **非商用ライセンス** | サイズ・ライセンスとも厳しい。見送り推奨 |
| NV-Embed-v2 | 7.8B | 4096 | 英語系ベンチ最強 | 不可（GPU 前提） | 非商用 | ブラウザ/CLI とも非現実的。見送り |

その他検討余地: `multilingual-e5-base`（現行の穏当な強化、768d）、
`static-retrieval-mrl-en-ja`（静的埋め込み、極小・爆速だが精度は落ちる）。

## 3. 選定方法 — MTEB ではなく実コーパスで測る

v0.1.5-rc3 で入った評価基盤をそのまま使う:

1. `scripts/self_improve.py` が蓄積した言い換えクエリ（`self_improve_work/paraphrases.json`）
   + 原文 `questions[]` を評価セットにする（一晩回せば数百〜千ケース規模になる）
2. 候補モデルごとに Node ワンオフスクリプトで
   - コーパス 220 件の `passage` 埋め込み
   - 全評価クエリの埋め込み
   を計算し、cosine ランキングの top1 / MRR を算出（`physq eval` の semantic 単体と同じ指標）
3. **現行 e5-small / e5-large をベースラインに、上回った候補だけを次段階へ**
4. ブラウザ実測（モデル DL サイズ・初回ロード時間・1 クエリの埋め込み時間）を
   iPhone/Android 実機で確認 — 学校ネットワークでの体感が最終関門

## 4. 移行手順（採用モデル決定後）

前提: `embeddings.json` のモデル別キー構造と `version.json` の `embedding_model` タグは
このための布石がすでにある。

1. **build.js**: `MODELS` に新キー（例 `"gemma"`）を追加し、**プレフィックス規約を
   モデル別に定義**する（重要 — 現行の `passage:`/`query:` は e5 専用。
   EmbeddingGemma は `title: none | text: ...`/`task: search result | query: ...`、
   Ruri は `検索文書: `/`検索クエリ: `、Qwen3 は instruction 形式、BGE-M3 は無印）。
   移行期間中は旧 e5 キーも並行生成
2. **schema_version を 4 に上げ**、`version.json` の `embedding_model` を新タグへ。
   physq は §5 のハッシュ比較で自動的に再取得する
3. **physq**: `ModelSize` 列挙に新モデルを追加（fastembed 対応外なら
   `UserDefinedEmbeddingModel` で ONNX+tokenizer を直接指定）。
   `semantic::query_text()` のプレフィックスをモデル別に分岐。
   旧バイナリが新 embeddings.json を読んでも `Invariant` で安全に落ちることを確認
4. **search.html / debug_search.html**: transformers.js のバージョンを必要版へ更新
   （CDN + SRI ハッシュ固定のパターン維持）、モデル ID とプレフィックスを差し替え
5. **検証**: `physq/scripts/parity_check.js` + `physq eval --cases`（評価セット全量）で
   移行前後の top1/MRR を比較し、レポートを残す
6. **後片付け**: 全クライアントが新版になったら旧キーの生成を止めて
   `embeddings.json` をスリム化（`physq update` の告知期間を挟む）

## 5. リスクと注意

- **プレフィックス／プーリングの取り違えが最大の事故要因**（physq CLAUDE.md §7 と同種の
  parity 問題が再発する）。モデルごとの規約を build.js / search.html / physq の
  3 か所で必ず同一にし、`real_data_tests.rs` に新モデルの self-consistency テストを足す
- ブラウザのモデル DL は毎ユーザー負担。EmbeddingGemma でも q4/q8 で 100–200 MB 級なので、
  **Web だけ旧 e5-small を残す「片側移行」も撤退線として保持**（その場合のみ 2 系統運用）
- `ort` は Intel Mac 向けバイナリを配らない制約が既にある（release workflow コメント参照）。
  新モデルでも同じ制約のままか確認
- Gemma 利用規約は Apache-2.0 ではない。再配布形態（GitHub Releases にモデルは同梱しない、
  実行時 DL のみ）なら問題になりにくいが、NOTICE への追記要否を確認する

## 6. 推奨（現時点の仮説）

**第一候補: EmbeddingGemma-300m（768d、MRL で 512d に切詰めて運用）を Web/CLI 共有。**
Qwen3-Embedding-0.6B は CLI 専用として魅力があるが、2 系統運用のコストが勝つ。
日本語クエリが圧倒的多数だと実測で分かれば Ruri v3-310m を再浮上させる。
いずれも §3 の実コーパス測定で e5 ベースラインに勝てなければ移行しない
（e5-large は今でも十分強い — 移行それ自体を目的化しない）。
