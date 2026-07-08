#!/usr/bin/env python3
# =============================================================
# Copyright 2026 Igarin & Legrs
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
# =============================================================
"""LM Studio のローカル LLM × physq eval による検索自己改善ループ。

「Mac を一晩放置するだけで検索が改善する」を目標に、以下を自動で繰り返す:

 1. 言い換え生成 — 現在全ケース合格しているレコードに対して、LLM が
    「同じ意味の言い換え質問」を n 個ずつ生成（検索の弱点を探す）
 2. 評価 — `physq eval --serve`（常駐プロセス、モデルロードは一晩で1回、
    クエリ埋め込みはメモリキャッシュ）で全ケース（原文 + 蓄積した言い換え）
    の各手法（BM25 / e5-small / e5-large / hybrid RRF）順位を測定
 3. 改善提案 — hybrid 1位を取れなかったレコードについて、LLM が
    keywords / synonyms の追加を提案し、最悪の言い換えを questions[] に追加
 4. 採否判定 — 提案を terms（keywords/synonyms）と questions の2パートに分け、
    それぞれ単独で適用 → search_text 再生成（node scripts/build.js --data）→
    再評価し、レコード単位で実際に改善したパートだけ採用（効いていない語が
    良い編集に相乗りするのを防ぐ）。編集していないレコードの成績が下がったら
    そのサイクルの編集を全て破棄。3サイクル直せない言い換えは隔離

安全性:
 - 編集対象は questions[1+] / keywords / synonyms への「追加」のみ。
   embeddings.json が参照する questions[0]・description には一切触れないため、
   埋め込みの再計算は不要（リポジトリの embeddings.json をそのまま使う）
 - 作業はすべて workdir（既定: self_improve_work/）内のコピーに対して行い、
   本番 q_and_a_data.json へは --apply / --apply-only で明示的に反映する
 - すべての状態をチェックポイント保存するので、Ctrl-C・クラッシュ後に
   同じコマンドで再開できる

使い方:
  # 動作確認（5レコード・1サイクルだけ）
  python3 scripts/self_improve.py --model qwen/qwen3-1.7b --records 5 --cycles 1

  # 一晩コース（8時間で自動停止、最後に重みチューニングも実施）
  python3 scripts/self_improve.py --model gemma-4-12b-it-mlx --hours 8 --tune-weights

  # 結果を本番データへ反映（レポート確認後に）
  python3 scripts/self_improve.py --apply-only
"""

from __future__ import annotations

import argparse
import copy
import json
import random
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
DATASET_NAME = "q_and_a_data.json"
METHODS = ("bm25", "small", "large", "hybrid")


class TimeUp(Exception):
    """--hours の期限に達した（正常終了扱い）。"""


def log(msg: str) -> None:
    print(f"[{datetime.now().strftime('%H:%M:%S')}] {msg}", flush=True)


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def load_json(path: Path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def save_json(path: Path, obj, indent=4) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(obj, f, ensure_ascii=False, indent=indent)
        f.write("\n")
    tmp.replace(path)


# ── LLM 出力からの JSON 抽出 ────────────────────────────────
def _strip_think(text: str) -> str:
    """qwen3 等の thinking モデルが出す <think>…</think> を除去する。"""
    return re.sub(r"<think>.*?</think>", "", text, flags=re.S)


def _balanced_json(text: str, open_ch: str, close_ch: str):
    """文字列リテラルを考慮して最初の対応の取れた JSON 構造を探す。"""
    start = text.find(open_ch)
    while start != -1:
        depth = 0
        in_str = False
        esc = False
        for i in range(start, len(text)):
            c = text[i]
            if esc:
                esc = False
                continue
            if c == "\\":
                esc = True
                continue
            if c == '"':
                in_str = not in_str
                continue
            if in_str:
                continue
            if c == open_ch:
                depth += 1
            elif c == close_ch:
                depth -= 1
                if depth == 0:
                    try:
                        return json.loads(text[start : i + 1])
                    except json.JSONDecodeError:
                        break
        start = text.find(open_ch, start + 1)
    return None


def extract_json_array(text: str):
    v = _balanced_json(_strip_think(text), "[", "]")
    return v if isinstance(v, list) else None


def extract_json_object(text: str):
    v = _balanced_json(_strip_think(text), "{", "}")
    return v if isinstance(v, dict) else None


# ── LM Studio クライアント（OpenAI 互換 API、stdlib のみ）──────
class LMStudio:
    def __init__(self, server: str, model: str, timeout: float = 300.0,
                 max_tokens: int = 4096):
        self.base = server.rstrip("/")
        self.model = model
        self.timeout = timeout
        # thinking モデル（Qwen 等）は <think> で大量にトークンを使うので、
        # 小さすぎると本文の JSON が出る前に切れる
        self.max_tokens = max_tokens
        self.calls = 0
        self.failures = 0

    def list_models(self) -> list[str]:
        req = urllib.request.Request(f"{self.base}/v1/models")
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.load(resp)
        return [m.get("id", "") for m in data.get("data", [])]

    def chat(self, system: str, user: str, temperature: float) -> str:
        payload = {
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "temperature": temperature,
            "max_tokens": self.max_tokens,
            "stream": False,
        }
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base}/v1/chat/completions",
            data=body,
            headers={"Content-Type": "application/json"},
        )
        last_err: Exception | None = None
        for attempt in range(3):
            try:
                self.calls += 1
                with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                    data = json.load(resp)
                return data["choices"][0]["message"]["content"]
            except (urllib.error.URLError, TimeoutError, KeyError, json.JSONDecodeError) as e:
                last_err = e
                self.failures += 1
                wait = 5 * (attempt + 1)
                log(f"  LLM 呼び出し失敗 ({e}); {wait}s 後にリトライ")
                time.sleep(wait)
        raise RuntimeError(f"LM Studio への接続に失敗しました: {last_err}")


# ── physq eval --serve ラッパ ───────────────────────────────
class EvalServer:
    def __init__(self, physq: Path, data: Path, embeddings: Path, model: str, log_path: Path):
        self.log_path = log_path
        self._log = open(log_path, "a", encoding="utf-8")
        cmd = [
            str(physq), "--model", model,
            "eval", "--serve",
            "--data", str(data), "--embeddings", str(embeddings), "--top", "3",
        ]
        self.proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self._log,
            text=True,
            encoding="utf-8",
        )
        ready = self._read()
        if ready.get("type") != "ready":
            raise RuntimeError(f"physq eval の起動応答が不正です: {ready}")
        self.ready = ready

    def _read(self) -> dict:
        line = self.proc.stdout.readline()
        if not line:
            code = self.proc.poll()
            raise RuntimeError(
                f"physq eval サーバが終了しました (exit={code})。ログ: {self.log_path}"
            )
        return json.loads(line)

    def rpc(self, obj: dict) -> dict:
        self.proc.stdin.write(json.dumps(obj, ensure_ascii=False) + "\n")
        self.proc.stdin.flush()
        return self._read()

    def evaluate(self, cases: list[dict], label: str = "") -> list[dict]:
        results = []
        for i, c in enumerate(cases):
            r = self.rpc({"id": c["id"], "query": c["query"], "target": c["target"]})
            if r.get("type") != "result":
                raise RuntimeError(f"評価エラー (case {c['id']}): {r}")
            results.append(r)
            if (i + 1) % 500 == 0:
                log(f"  eval{label}: {i + 1}/{len(cases)}")
        return results

    def reload(self, path: Path) -> None:
        r = self.rpc({"cmd": "reload_data", "path": str(path)})
        if r.get("type") != "ok":
            raise RuntimeError(f"reload_data 失敗: {r}")

    def set_weights(self, bm25: float, small: float, large: float) -> None:
        r = self.rpc({"cmd": "weights", "bm25": bm25, "small": small, "large": large})
        if r.get("type") != "ok":
            raise RuntimeError(f"weights 失敗: {r}")

    def close(self) -> None:
        try:
            self.proc.stdin.close()
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()
        self._log.close()


# ── 指標計算 ────────────────────────────────────────────────
def summarize(results: list[dict]) -> dict:
    """手法ごとの top1 / top1_rate / MRR。"""
    out = {}
    for m in METHODS:
        ranks = [r["methods"][m]["rank"] for r in results if m in r["methods"]]
        if not ranks:
            continue
        n = len(ranks)
        top1 = sum(1 for x in ranks if x == 1)
        mrr = sum(1.0 / x for x in ranks if x) / n
        out[m] = {"cases": n, "top1": top1, "top1_rate": top1 / n, "mrr": mrr}
    return out


def per_record(results: list[dict]) -> dict:
    """レコード（target）ごとの hybrid 成績: {rid: {n, top1, rr_sum}}。"""
    acc: dict[str, dict] = {}
    for r in results:
        rid = r["target"]
        rank = r["methods"]["hybrid"]["rank"]
        a = acc.setdefault(rid, {"n": 0, "top1": 0, "rr_sum": 0.0})
        a["n"] += 1
        if rank == 1:
            a["top1"] += 1
        if rank:
            a["rr_sum"] += 1.0 / rank
    return acc


def record_improved(before: dict, after: dict) -> bool:
    """編集がそのレコードのケース群を改善したか（同数なら不採用）。"""
    if after["top1"] != before["top1"]:
        return after["top1"] > before["top1"]
    return after["rr_sum"] > before["rr_sum"] + 1e-9


# ── テキスト検証 ────────────────────────────────────────────
def norm_q(s: str) -> str:
    return re.sub(r"[\s、。，．？?！!]+", "", s).lower()


def valid_paraphrase(s: str, existing_norms: set[str]) -> bool:
    s = s.strip()
    if not (2 <= len(s) <= 100):
        return False
    if "\n" in s or "$" in s or "\\" in s:
        return False
    return norm_q(s) not in existing_norms


def valid_term(s: str, existing_lower: set[str]) -> bool:
    s = s.strip()
    if not (1 <= len(s) <= 25):
        return False
    if "\n" in s or "$" in s or "\\" in s:
        return False
    return s.lower() not in existing_lower


# ── LLM プロンプト ──────────────────────────────────────────
# 中規模ローカルモデル（Qwen / Gemma 等の instruct、4bit量子化を含む）でも
# 出力が安定するよう、(1) 役割と目的、(2) 具体例つきのルール、(3) 厳密な
# 出力契約、の3点を明示する。thinking モデルの <think> は抽出側で除去される。
SYSTEM = (
    "あなたは高校物理のQ&A検索システムを改善するためのデータ作成アシスタントです。"
    "回答は必ず指示された形式のJSONだけを出力してください。"
    "説明文・前置き・言い訳・マークダウンのコードフェンスは一切出力してはいけません。"
)


def gen_paraphrases(llm: LMStudio, rec: dict, existing: list[str], n: int, temp: float) -> list[str]:
    q0 = (rec.get("questions") or [""])[0]
    desc = (rec.get("description") or "")[:200]
    shown = existing[-20:]  # プロンプト肥大防止
    user = f"""高校物理のQ&Aサイトの「元の質問」に対して、同じ答えにたどり着くべき言い換え検索クエリを{n}個作ってください。

# 目的
生徒は同じ疑問をさまざまな表現で検索窓に打ち込みます。検索エンジンがどんな表現でも
この質問のQ&Aを1位に出せるかテストするための、現実的な検索クエリを作ります。

# 元の質問
{q0}

# このQ&Aの補足説明（質問の意味を正しく理解するための参考情報）
{desc}

# 作り方のルール
1. 「元の質問」と同じ疑問・同じ答えを指すこと。話題を広げたり狭めたりしない
   （悪い例: 元が「電位とは何か」なのに「電圧とは何か」を作る — 別の概念なので不可）
2. {n}個は互いに表現の型を大きく変えること。次の型を織り交ぜる:
   - 口語調: 「〜ってどういうこと？」「なんで〜なの？」
   - 体言止め・名詞句: 「〜の理由」「〜の仕組み」
   - 検索キーワードの羅列: 「電磁誘導 原理」のような2〜3語
   - 同じ意味の別の言い回し・かな書きを使った形
3. 元の質問が日本語なら日本語で、英語なら英語で書く
4. 数式や記号（$・バックスラッシュ）は使わない
5. 次の既出リストと同じ・ほぼ同じ表現は作らない:
{json.dumps(shown, ensure_ascii=False)}

# 出力形式
JSON文字列配列のみを出力する。他のテキストは一切出力しない。
["言い換え1", "言い換え2", ...]

# 良い出力の例（元の質問が「なぜ夕焼けは赤いのか？」だった場合）
["夕焼けが赤く見える理由", "夕日 赤い なぜ", "夕方の空はどうして赤色になるの？", "空の色 夕方 変わる仕組み"]"""
    text = llm.chat(SYSTEM, user, temperature=temp)
    arr = extract_json_array(text)
    if arr is None:
        return []
    existing_norms = {norm_q(x) for x in existing}
    existing_norms |= {norm_q(q) for q in rec.get("questions") or []}
    out: list[str] = []
    for item in arr:
        if not isinstance(item, str):
            continue
        s = item.strip()
        if valid_paraphrase(s, existing_norms | {norm_q(o) for o in out}):
            out.append(s)
        if len(out) >= n:
            break
    return out


def propose_terms(llm: LMStudio, rec: dict, fails: list[dict], by_id: dict) -> dict:
    """失敗ケースから keywords/synonyms 追加案を LLM に出させる。"""
    q0 = (rec.get("questions") or [""])[0]
    desc = (rec.get("description") or "")[:300]
    lines = []
    for f in fails[:5]:
        rank = f["methods"]["hybrid"]["rank"]
        top1_id = f["top"][0]["id"] if f.get("top") else "?"
        top1_q = (by_id.get(top1_id, {}).get("questions") or ["?"])[0]
        lines.append(f'- 「{f["query"]}」 → 正解は現在{rank if rank else "圏外"}位。'
                     f'かわりに1位だったのは「{top1_q[:60]}」')
    kw = json.dumps(rec.get("keywords") or [], ensure_ascii=False)
    syn = json.dumps(rec.get("synonyms") or [], ensure_ascii=False)
    user = f"""高校物理のQ&A検索システムで、下のクエリに対して本来1位に出るべきレコード（正解レコード）が1位になれませんでした。
正解レコードの検索用メタデータ（keywords / synonyms）に追加すべき語を提案してください。

# 正解レコード
- 代表質問: {q0}
- 説明: {desc}
- 現在の keywords: {kw}
- 現在の synonyms: {syn}

# 1位になれなかったクエリ
{chr(10).join(lines)}

# 提案のルール
1. クエリに含まれる言葉のうち、正解レコードのメタデータにまだ無い語を最優先で拾う
2. 正解レコードの内容を的確に表す語だけを選ぶ。次のような語は禁止:
   - 「かわりに1位だったレコード」の内容を指す語（そちらを強化してしまい逆効果）
   - 「物理」「公式」「理由」「仕組み」のような、多くのQ&Aに当てはまる一般的すぎる語
3. keywords には概念・現象・用語を入れる（例: 電磁誘導、レンツの法則）
   synonyms には同じものの別の呼び方・別表記・かな書きを入れる（例: 誘導起電力 と ゆうどうきでんりょく）
4. 各語は1〜20文字。現在の keywords / synonyms と重複させない
5. 確信が持てる語だけを出す。適切な候補がなければ空配列にする（無理に埋めない）
6. 最大 keywords 3個 / synonyms 3個

# 出力形式
JSONオブジェクトのみを出力する。他のテキストは一切出力しない。
{{"keywords": ["..."], "synonyms": ["..."]}}"""
    text = llm.chat(SYSTEM, user, temperature=0.3)
    obj = extract_json_object(text) or {}
    existing = {k.lower() for k in (rec.get("keywords") or [])}
    existing |= {s.lower() for s in (rec.get("synonyms") or [])}
    out = {"keywords": [], "synonyms": []}
    for field in ("keywords", "synonyms"):
        vals = obj.get(field)
        if not isinstance(vals, list):
            continue
        for v in vals:
            if isinstance(v, str) and valid_term(v.strip(), existing):
                term = v.strip()
                out[field].append(term)
                existing.add(term.lower())
            if len(out[field]) >= 3:
                break
    return out


# ── データセット編集 ────────────────────────────────────────
def apply_edit(rec: dict, edit: dict) -> None:
    rec.setdefault("keywords", []).extend(edit.get("keywords", []))
    rec.setdefault("synonyms", []).extend(edit.get("synonyms", []))
    rec.setdefault("questions", []).extend(edit.get("questions", []))


# ── メインの文脈オブジェクト ────────────────────────────────
class Ctx:
    def __init__(self, args):
        self.args = args
        self.workdir = Path(args.workdir).resolve()
        self.workdir.mkdir(parents=True, exist_ok=True)
        self.dataset_path = self.workdir / "dataset.json"
        self.paraphrases_path = self.workdir / "paraphrases.json"
        self.state_path = self.workdir / "state.json"
        self.history_path = self.workdir / "history.jsonl"
        self.report_path = self.workdir / "report.md"
        self.deadline = time.time() + args.hours * 3600 if args.hours else None
        self.llm: LMStudio | None = None
        self.server: EvalServer | None = None
        self.paraphrases: dict[str, list[dict]] = {}
        self.state: dict = {}

    def check_deadline(self) -> None:
        if self.deadline and time.time() > self.deadline:
            raise TimeUp()

    def checkpoint(self) -> None:
        save_json(self.paraphrases_path, self.paraphrases, indent=1)
        save_json(self.state_path, self.state, indent=1)

    def history(self, event: str, **kw) -> None:
        row = {"ts": now_iso(), "event": event, **kw}
        with open(self.history_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")


def find_physq(arg: str | None) -> Path:
    if arg:
        p = Path(arg)
        if p.exists():
            return p
        sys.exit(f"--physq {arg} が見つかりません")
    for cand in (
        REPO / "physq/target/release/physq",
        REPO / "physq/target/debug/physq",
    ):
        if cand.exists():
            return cand
    which = shutil.which("physq")
    if which:
        return Path(which)
    sys.exit(
        "physq バイナリが見つかりません。physq/ で `cargo build --release` するか "
        "--physq でパスを指定してください（eval サブコマンド対応の v0.1.5-rc3 以降が必要）"
    )


# e5-small (fp32) ~470MB, e5-large (fp32, 量子化なし) ~2.1GB。fastembed の
# モデルキャッシュ（未ダウンロードなら）+ 作業用データの余裕を見て閾値を決める。
_DISK_MIN_GB = {"none": 1.0, "small": 2.0, "large": 4.0, "max": 5.0}


def check_disk_space(workdir: Path, eval_model: str) -> None:
    """ディスク逼迫時、モデルダウンロード/ロードが極端に遅くなる（数分〜）前に警告する。"""
    try:
        free_gb = shutil.disk_usage(workdir).free / (1024 ** 3)
    except OSError:
        return
    need = _DISK_MIN_GB.get(eval_model, 2.0)
    if free_gb < need:
        log(
            f"警告: 空きディスクが {free_gb:.1f}GB しかありません "
            f"(--eval-model {eval_model} の目安 {need}GB 以上を推奨)。"
            "空きが少ないと e5-large 等のモデル読み込みが数分単位で遅くなり、"
            "「固まった」ように見えることがあります。"
            "空き容量を確保するか、--eval-model small で e5-large のダウンロードを"
            "回避することを検討してください"
        )


def regen_search_text(ctx: Ctx) -> None:
    """作業コピーの search_text を再生成（kuromoji は repo ルートの node_modules）。"""
    r = subprocess.run(
        ["node", "scripts/build.js", "--data", str(ctx.dataset_path)],
        cwd=REPO, capture_output=True, text=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"build.js --data 失敗:\n{r.stdout}\n{r.stderr}")


def build_cases(scope: list[dict], paraphrases: dict) -> list[dict]:
    cases = []
    for rec in scope:
        rid = rec["id"]
        for i, q in enumerate(rec.get("questions") or []):
            cases.append({"id": f"{rid}|orig{i}", "query": q, "target": rid})
        for i, p in enumerate(paraphrases.get(rid, [])):
            if p.get("quarantined"):
                continue  # 3サイクル直せなかった言い換え（低品質の可能性）は除外
            cases.append({"id": f"{rid}|para{i}", "query": p["q"], "target": rid})
    return cases


def scope_records(data: list[dict], args) -> list[dict]:
    return data[: args.records] if args.records else data


# ── 1 サイクル ──────────────────────────────────────────────
def run_cycle(ctx: Ctx, cycle: int) -> dict:
    args = ctx.args
    data = load_json(ctx.dataset_path)
    by_id = {r["id"]: r for r in data}
    scope = scope_records(data, args)
    status = ctx.state.get("record_status", {})

    # 1. 言い換え生成（前回全勝レコードに新しい挑戦者を作る。失敗中のレコードは
    #    まず今の失敗ケースを直すのが先なのでスキップ）
    gen_targets = [
        r for r in scope
        if status.get(r["id"], "pass") == "pass"
        and len(ctx.paraphrases.get(r["id"], [])) < args.max_paraphrases
        and (r.get("questions") or [""])[0].strip()
    ]
    log(f"cycle {cycle}: 言い換え生成 {len(gen_targets)} レコード × {args.paraphrases} 個")
    for i, rec in enumerate(gen_targets):
        ctx.check_deadline()
        rid = rec["id"]
        existing = [p["q"] for p in ctx.paraphrases.get(rid, [])]
        news = gen_paraphrases(ctx.llm, rec, existing, args.paraphrases, args.temperature)
        ctx.paraphrases.setdefault(rid, []).extend(
            {"q": q, "cycle": cycle} for q in news
        )
        if (i + 1) % 10 == 0:
            ctx.checkpoint()
            log(f"  言い換え生成 {i + 1}/{len(gen_targets)}")
    ctx.checkpoint()

    # 2. ベースライン評価
    cases = build_cases(scope, ctx.paraphrases)
    log(f"cycle {cycle}: 評価 {len(cases)} ケース（ベースライン）")
    baseline = ctx.server.evaluate(cases, label="(base)")
    base_sum = summarize(baseline)
    base_rec = per_record(baseline)
    log(f"  hybrid top1 {base_sum['hybrid']['top1']}/{base_sum['hybrid']['cases']} "
        f"({base_sum['hybrid']['top1_rate']:.1%}), MRR {base_sum['hybrid']['mrr']:.4f}")

    # 3. 失敗レコードへの改善提案
    fails_by_rid: dict[str, list[dict]] = {}
    for r in baseline:
        if r["methods"]["hybrid"]["rank"] != 1:
            fails_by_rid.setdefault(r["target"], []).append(r)
    accepted_state = ctx.state.setdefault("accepted", {})
    cooldowns = ctx.state.setdefault("cooldown_until", {})
    # 提案は「terms（keywords/synonyms）」と「questions（言い換えの登録）」の
    # 2パートに分け、まず別々に適用して効果を測る。まとめて試すと、効いて
    # いない語（LLM の幻覚など）が同じレコードの良い編集に相乗りして混入する。
    proposals: dict[str, dict] = {}  # rid -> {"terms": edit|None, "questions": edit|None}
    log(f"cycle {cycle}: 改善提案（失敗レコード {len(fails_by_rid)} 件）")
    for rid, fails in fails_by_rid.items():
        ctx.check_deadline()
        if cooldowns.get(rid, 0) > cycle:
            continue  # 直せなかったレコードはしばらく休ませる
        rec = by_id[rid]
        got = accepted_state.get(rid, {})
        kw_room = args.max_new_keywords - len(got.get("keywords", []))
        syn_room = args.max_new_synonyms - len(got.get("synonyms", []))
        q_room = args.max_new_questions - len(got.get("questions", []))

        terms_edit = None
        if kw_room > 0 or syn_room > 0:
            terms = propose_terms(ctx.llm, rec, fails, by_id)
            kw = terms["keywords"][:max(kw_room, 0)]
            syn = terms["synonyms"][:max(syn_room, 0)]
            if kw or syn:
                terms_edit = {"keywords": kw, "synonyms": syn}

        # 最悪の言い換えクエリを questions[] へ追加(検索側に実例を教える)。
        # questions[0] は embeddings が参照するので絶対に触らない＝追加のみ。
        quest_edit = None
        if q_room > 0:
            para_fails = [f for f in fails if "|para" in f["id"]]
            if para_fails:
                worst = max(
                    para_fails,
                    key=lambda f: f["methods"]["hybrid"]["rank"] or 10 ** 9,
                )
                qnorms = {norm_q(q) for q in rec.get("questions") or []}
                if norm_q(worst["query"]) not in qnorms:
                    quest_edit = {"questions": [worst["query"]]}

        if terms_edit or quest_edit:
            proposals[rid] = {"terms": terms_edit, "questions": quest_edit}

    accepted: dict[str, dict] = {}
    final_results = baseline
    final_sum = base_sum
    if proposals:
        baseline_data = copy.deepcopy(data)
        zero = {"top1": 0, "rr_sum": 0.0}

        def trial(parts: dict[str, dict], label: str) -> list[dict]:
            """baseline に parts だけを適用した状態を作って全ケースを評価。"""
            tdata = copy.deepcopy(baseline_data)
            tb = {r["id"]: r for r in tdata}
            for trid, e in parts.items():
                apply_edit(tb[trid], e)
            save_json(ctx.dataset_path, tdata)
            regen_search_text(ctx)
            ctx.server.reload(ctx.dataset_path)
            return ctx.server.evaluate(cases, label=label)

        def merge_part(dst: dict, e: dict) -> None:
            for f in ("keywords", "synonyms", "questions"):
                if e.get(f):
                    dst.setdefault(f, []).extend(e[f])

        # 4. パート別トライアル: 単独で効いたパートだけを候補に残す
        term_parts = {rid: p["terms"] for rid, p in proposals.items() if p["terms"]}
        quest_parts = {rid: p["questions"] for rid, p in proposals.items() if p["questions"]}
        if term_parts:
            log(f"cycle {cycle}: terms 提案 {len(term_parts)} 件を単独評価")
            rec_t = per_record(trial(term_parts, "(terms)"))
            for rid, e in term_parts.items():
                if record_improved(base_rec.get(rid, zero), rec_t.get(rid, zero)):
                    merge_part(accepted.setdefault(rid, {}), e)
        if quest_parts:
            log(f"cycle {cycle}: questions 提案 {len(quest_parts)} 件を単独評価")
            rec_q = per_record(trial(quest_parts, "(quest)"))
            for rid, e in quest_parts.items():
                if record_improved(base_rec.get(rid, zero), rec_q.get(rid, zero)):
                    merge_part(accepted.setdefault(rid, {}), e)

        # 単独では効かなくても両パート併用なら効くかもしれないレコードは、
        # 最終評価に同乗させて判定する（追加の評価コストゼロ）
        combo = {
            rid: p for rid, p in proposals.items()
            if rid not in accepted and p["terms"] and p["questions"]
        }
        trial_parts = {rid: dict(e) for rid, e in accepted.items()}
        for rid, p in combo.items():
            merged: dict = {}
            merge_part(merged, p["terms"])
            merge_part(merged, p["questions"])
            trial_parts[rid] = merged

        # 5. 採用候補をまとめて適用し、レコード単位で最終判定
        if trial_parts:
            log(f"cycle {cycle}: 採用候補 {len(trial_parts)} 件で最終評価")
            final_results = trial(trial_parts, "(final)")
            final_rec = per_record(final_results)
            drop = [
                rid for rid in trial_parts
                if not record_improved(base_rec.get(rid, zero), final_rec.get(rid, zero))
            ]
            if drop:
                log(f"cycle {cycle}: 併用評価で {len(drop)} 件を巻き戻し")
                for rid in drop:
                    trial_parts.pop(rid)
                if trial_parts:
                    final_results = trial(trial_parts, "(final2)")
                else:
                    save_json(ctx.dataset_path, baseline_data)
                    regen_search_text(ctx)
                    ctx.server.reload(ctx.dataset_path)
                    final_results = baseline
            accepted = trial_parts
            final_sum = summarize(final_results)
            final_rec = per_record(final_results)

            # 6. 全体ガード: 全体指標の悪化、または編集していないレコードの
            #    悪化があればこのサイクルの編集を全て破棄（夜間の暴走防止）
            side_effect = [
                rid for rid, b in base_rec.items()
                if rid not in accepted and final_rec.get(rid, b)["top1"] < b["top1"]
            ]
            globally_worse = (
                final_sum["hybrid"]["top1"] < base_sum["hybrid"]["top1"]
                or (final_sum["hybrid"]["top1"] == base_sum["hybrid"]["top1"]
                    and final_sum["hybrid"]["mrr"] < base_sum["hybrid"]["mrr"] - 1e-9)
            )
            if accepted and (side_effect or globally_worse):
                reason = f"副作用レコード {side_effect}" if side_effect else "全体指標の悪化"
                log(f"cycle {cycle}: {reason} のため全編集を破棄")
                save_json(ctx.dataset_path, baseline_data)
                regen_search_text(ctx)
                ctx.server.reload(ctx.dataset_path)
                ctx.history("cycle_reverted_all", cycle=cycle, reason=reason,
                            side_effect=side_effect)
                accepted = {}
                final_results = baseline
                final_sum = base_sum
        else:
            # トライアルで作業コピーが書き換わっているので必ず baseline へ戻す
            save_json(ctx.dataset_path, baseline_data)
            regen_search_text(ctx)
            ctx.server.reload(ctx.dataset_path)
            final_results = baseline
            final_sum = base_sum
    else:
        log(f"cycle {cycle}: 提案なし（全ケース合格 or 休止中 or 上限到達）")

    # 採用結果を累積状態へ
    for rid, edit in accepted.items():
        got = accepted_state.setdefault(
            rid, {"keywords": [], "synonyms": [], "questions": []}
        )
        for f in ("keywords", "synonyms", "questions"):
            got[f].extend(edit.get(f, []))
        ctx.history("edit_accepted", cycle=cycle, record=rid, edit=edit)

    # 7. 修正試行の記録と隔離: 3サイクル連続で改善できなかった言い換えは
    #    低品質（意味のずれた生成）の可能性が高いので評価セットから外す。
    #    原文質問しか失敗していないレコードはしばらく休ませて LLM 呼び出しを節約。
    final_fails: dict[str, list[dict]] = {}
    for r in final_results:
        if r["methods"]["hybrid"]["rank"] != 1:
            final_fails.setdefault(r["target"], []).append(r)
    attempts = ctx.state.setdefault("fix_attempts", {})
    for rid in list(attempts):
        if rid not in final_fails:
            del attempts[rid]  # 直ったのでリセット
    for rid in fails_by_rid:
        if rid not in final_fails or rid in accepted:
            continue  # 直った / 改善が進んでいる間はカウントしない
        if cooldowns.get(rid, 0) > cycle:
            continue  # このサイクルでは修正を試みていない
        attempts[rid] = attempts.get(rid, 0) + 1
        if attempts[rid] < 3:
            continue
        attempts[rid] = 0
        para_fails = [f for f in final_fails[rid] if "|para" in f["id"]]
        if para_fails:
            for f in para_fails:
                idx = int(f["id"].rsplit("|para", 1)[1])
                ctx.paraphrases[rid][idx]["quarantined"] = True
            log(f"  {rid[:8]}: 3サイクル直せなかった言い換え {len(para_fails)} 件を隔離")
            ctx.history("paraphrases_quarantined", cycle=cycle, record=rid,
                        queries=[f["query"] for f in para_fails])
        else:
            cooldowns[rid] = cycle + 5
            log(f"  {rid[:8]}: 原文質問を直せないため cycle {cooldowns[rid]} まで保留")
            ctx.history("record_cooldown", cycle=cycle, record=rid, until=cooldowns[rid])

    update_status(ctx, final_results)
    stats = {
        "cycle": cycle, "cases": len(cases), "before": base_sum, "after": final_sum,
        "proposed": len(proposals), "accepted": len(accepted),
        "reverted": len(proposals) - len(accepted),
    }
    log(f"cycle {cycle}: 採用 {stats['accepted']}/{stats['proposed']} — "
        f"hybrid top1 {base_sum['hybrid']['top1']} → {final_sum['hybrid']['top1']}, "
        f"MRR {base_sum['hybrid']['mrr']:.4f} → {final_sum['hybrid']['mrr']:.4f}")
    return stats


def update_status(ctx: Ctx, results: list[dict]) -> None:
    status: dict[str, str] = {}
    for r in results:
        rid = r["target"]
        if r["methods"]["hybrid"]["rank"] != 1:
            status[rid] = "fail"
        else:
            status.setdefault(rid, "pass")
    ctx.state["record_status"] = status


# ── RRF 重みチューニング（レポートのみ、physq 既定値は変えない）──
def tune_weights(ctx: Ctx) -> dict | None:
    data = load_json(ctx.dataset_path)
    cases = build_cases(scope_records(data, ctx.args), ctx.paraphrases)
    rng = random.Random(ctx.args.seed)
    if len(cases) > 2000:
        cases = rng.sample(cases, 2000)
    log(f"重みチューニング開始（{len(cases)} ケース、座標降下 2 パス）")

    grid = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0]
    best = [1.0, 2.0, 2.0]

    def score(w):
        ctx.server.set_weights(*w)
        s = summarize(ctx.server.evaluate(cases, label="(tune)"))["hybrid"]
        return (s["top1"], s["mrr"])

    best_score = score(best)
    log(f"  既定 (1,2,2): top1={best_score[0]}, MRR={best_score[1]:.4f}")
    for _pass in range(2):
        for axis in range(3):
            for v in grid:
                ctx.check_deadline()
                w = list(best)
                w[axis] = v
                if w == best:
                    continue
                s = score(w)
                if s > best_score:
                    best, best_score = w, s
                    log(f"  改善: {tuple(best)} → top1={s[0]}, MRR={s[1]:.4f}")
    ctx.server.set_weights(1.0, 2.0, 2.0)  # 後続評価のため既定へ戻す
    result = {
        "weights": {"bm25": best[0], "small": best[1], "large": best[2]},
        "top1": best_score[0], "mrr": best_score[1], "cases": len(cases),
    }
    ctx.state["tuned_weights"] = result
    ctx.checkpoint()
    return result


# ── レポート ────────────────────────────────────────────────
def write_report(ctx: Ctx) -> None:
    st = ctx.state
    rows = st.get("history_rows", [])
    lines = [
        "# 検索自己改善レポート",
        "",
        f"- 最終更新: {now_iso()}",
        f"- LLM: `{ctx.args.model}` @ `{ctx.args.server}`",
        f"- eval: `physq eval --serve --model {ctx.args.eval_model}`",
        f"- 作業コピー: `{ctx.dataset_path}`",
        "",
        "## サイクル履歴（hybrid）",
        "",
        "| cycle | cases | top1 前→後 | MRR 前→後 | 提案 | 採用 | 巻戻し |",
        "|---|---|---|---|---|---|---|",
    ]
    for r in rows:
        b, a = r["before"]["hybrid"], r["after"]["hybrid"]
        lines.append(
            f"| {r['cycle']} | {r['cases']} "
            f"| {b['top1']} → {a['top1']} "
            f"| {b['mrr']:.4f} → {a['mrr']:.4f} "
            f"| {r['proposed']} | {r['accepted']} | {r['reverted']} |"
        )
    if rows:
        last = rows[-1]["after"]
        lines += ["", "## 最終スコア（手法別）", "",
                  "| method | top1 | top1率 | MRR |", "|---|---|---|---|"]
        for m in METHODS:
            if m in last:
                s = last[m]
                lines.append(
                    f"| {m} | {s['top1']}/{s['cases']} | {s['top1_rate']:.1%} | {s['mrr']:.4f} |"
                )
    accepted = st.get("accepted", {})
    total_kw = sum(len(e["keywords"]) for e in accepted.values())
    total_syn = sum(len(e["synonyms"]) for e in accepted.values())
    total_q = sum(len(e["questions"]) for e in accepted.values())
    lines += [
        "",
        f"## 採用された編集（{len(accepted)} レコード / "
        f"keywords +{total_kw}, synonyms +{total_syn}, questions +{total_q}）",
        "",
    ]
    dataset = {r["id"]: r for r in load_json(ctx.dataset_path)} if ctx.dataset_path.exists() else {}
    for rid, e in accepted.items():
        q0 = (dataset.get(rid, {}).get("questions") or ["?"])[0]
        parts = []
        if e["keywords"]:
            parts.append("keywords " + json.dumps(e["keywords"], ensure_ascii=False))
        if e["synonyms"]:
            parts.append("synonyms " + json.dumps(e["synonyms"], ensure_ascii=False))
        if e["questions"]:
            parts.append("questions " + json.dumps(e["questions"], ensure_ascii=False))
        lines.append(f"- `{rid}` 「{q0[:40]}」: +" + ", +".join(parts))
    tw = st.get("tuned_weights")
    if tw:
        w = tw["weights"]
        lines += [
            "",
            "## RRF 重みチューニング結果（参考値 — physq の既定値は変更していない）",
            "",
            f"- 最良: bm25={w['bm25']}, small={w['small']}, large={w['large']} "
            f"(top1={tw['top1']}, MRR={tw['mrr']:.4f}, {tw['cases']} ケース)",
            "- 既定 (1, 2, 2) から変える場合は physq `--debug` の custom モードで検証のうえ、",
            "  `physq/src/config.rs` の定数と `search.html` 側をどうするか判断すること。",
        ]
    lines += [
        "",
        "## 本番への反映",
        "",
        "```sh",
        "python3 scripts/self_improve.py --apply-only   # 作業コピーを q_and_a_data.json へ",
        "git diff q_and_a_data.json                     # 内容を確認してからコミット",
        "```",
        "",
    ]
    ctx.report_path.write_text("\n".join(lines), encoding="utf-8")


# ── 反映 ────────────────────────────────────────────────────
def apply_to_repo(ctx: Ctx) -> None:
    if not ctx.dataset_path.exists():
        sys.exit(f"作業コピーがありません: {ctx.dataset_path}")
    target = REPO / DATASET_NAME
    shutil.copyfile(ctx.dataset_path, target)
    log(f"{target} へ反映し、search_text / version.json を再生成します")
    r = subprocess.run(["node", "scripts/build.js"], cwd=REPO, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"build.js 失敗:\n{r.stdout}\n{r.stderr}")
    log("完了。`git diff q_and_a_data.json` で確認のうえコミットしてください")
    log("（push すれば GitHub Actions が embeddings.json / version.json を再生成・確定します）")


# ── main ────────────────────────────────────────────────────
def parse_args():
    ap = argparse.ArgumentParser(
        description="LM Studio の LLM で検索データセットを自律改善するループ",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    ap.add_argument("--server", default="http://localhost:1234", help="LM Studio サーバURL")
    ap.add_argument("--model", help="LLM モデルID（/v1/models に出る名前）")
    ap.add_argument("--eval-model", default="max", choices=["small", "large", "max"],
                    help="physq eval に使う埋め込みモデル")
    ap.add_argument("--paraphrases", type=int, default=4, help="1レコード1サイクルの言い換え数")
    ap.add_argument("--max-paraphrases", type=int, default=20, help="1レコードの言い換え上限")
    ap.add_argument("--max-new-keywords", type=int, default=8, help="1レコードの追加keywords上限")
    ap.add_argument("--max-new-synonyms", type=int, default=8, help="1レコードの追加synonyms上限")
    ap.add_argument("--max-new-questions", type=int, default=4, help="1レコードの追加questions上限")
    ap.add_argument("--cycles", type=int, help="サイクル数上限（省略時: --hours まで、両方省略なら1）")
    ap.add_argument("--hours", type=float, help="実行時間の上限（例: 一晩=8）")
    ap.add_argument("--records", type=int, help="先頭Nレコードだけ対象にする（動作確認用）")
    ap.add_argument("--temperature", type=float, default=0.9, help="言い換え生成の温度")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--physq", help="physq バイナリのパス（省略時は自動検出）")
    ap.add_argument("--workdir", default=str(REPO / "self_improve_work"), help="状態保存ディレクトリ")
    ap.add_argument("--llm-timeout", type=float, default=300, help="LLM 1呼び出しのタイムアウト秒")
    ap.add_argument("--llm-max-tokens", type=int, default=4096,
                    help="LLM 1呼び出しの最大出力トークン（thinkingモデルは大きめに）")
    ap.add_argument("--tune-weights", action="store_true",
                    help="終了時に RRF 重みの座標降下チューニングを行う（レポートのみ）")
    ap.add_argument("--apply", action="store_true",
                    help="終了時に作業コピーを本番 q_and_a_data.json へ反映する")
    ap.add_argument("--apply-only", action="store_true",
                    help="ループを回さず、既存の作業コピーを本番へ反映して終了")
    ap.add_argument("--fresh", action="store_true",
                    help="workdir の状態を破棄して最初からやり直す")
    return ap.parse_args()


def main() -> None:
    args = parse_args()
    ctx = Ctx(args)

    if args.apply_only:
        apply_to_repo(ctx)
        return

    if not args.model:
        sys.exit("--model を指定してください（LM Studio の /v1/models に出る名前）")

    if args.fresh:
        for p in (ctx.dataset_path, ctx.paraphrases_path, ctx.state_path,
                  ctx.history_path, ctx.report_path):
            p.unlink(missing_ok=True)

    # node / physq / LM Studio の存在確認
    if not shutil.which("node"):
        sys.exit("node が見つかりません（search_text 再生成に必要）")
    if not (REPO / "node_modules" / "kuromoji").exists():
        sys.exit(f"kuromoji が見つかりません。{REPO} で `npm install` してください")
    physq = find_physq(args.physq)
    ctx.llm = LMStudio(args.server, args.model, timeout=args.llm_timeout,
                       max_tokens=args.llm_max_tokens)
    try:
        models = ctx.llm.list_models()
    except Exception as e:
        sys.exit(f"LM Studio ({args.server}) に接続できません: {e}\n"
                 "LM Studio を起動し、開発者タブでサーバを有効にしてください")
    if args.model not in models:
        log(f"注意: モデル {args.model} が /v1/models に見つかりません "
            f"(available: {models})。JITロード設定なら初回呼び出しでロードされます")

    # 作業コピー準備（再開時は既存を維持）
    resumed = ctx.dataset_path.exists()
    if not resumed:
        shutil.copyfile(REPO / DATASET_NAME, ctx.dataset_path)
    if ctx.paraphrases_path.exists():
        ctx.paraphrases = load_json(ctx.paraphrases_path)
    if ctx.state_path.exists():
        ctx.state = load_json(ctx.state_path)
    ctx.state.setdefault("started", now_iso())
    ctx.state.setdefault("history_rows", [])
    log(f"作業コピー: {ctx.dataset_path}（{'再開' if resumed else '新規'}）")
    regen_search_text(ctx)  # search_text を最新化（通常は no-op）
    check_disk_space(ctx.workdir, args.eval_model)

    def _sigterm(*_):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, _sigterm)

    max_cycles = args.cycles if args.cycles else (10 ** 9 if args.hours else 1)
    start_cycle = ctx.state.get("cycle", 0) + 1
    try:
        # eval サーバの起動待ち（初回は e5-small ~470MB + e5-large ~2GB のダウンロード
        # 込みで数分かかることがある — 「固まった」ように見えても正常）もこの
        # try/finally の内側に置き、起動待ち中の Ctrl-C でも安全に終了できるようにする
        size_note = (
            "small+large 両モデル計 ~2.5GB を" if args.eval_model == "max"
            else "e5-large ~2GB を" if args.eval_model == "large" else "e5-small ~470MB を"
        )
        log(f"physq eval サーバ起動中（--model {args.eval_model}）… "
            f"初回は{size_note}ダウンロードするため数分かかることがあります（固まっていません）")
        ctx.server = EvalServer(
            physq, ctx.dataset_path, REPO / "embeddings.json",
            args.eval_model, ctx.workdir / "eval_server.log",
        )
        log(f"eval サーバ ready: {ctx.server.ready['records']} レコード, "
            f"models={ctx.server.ready['models']}")

        for cycle in range(start_cycle, start_cycle + max_cycles):
            ctx.check_deadline()
            ctx.state["cycle"] = cycle
            stats = run_cycle(ctx, cycle)
            ctx.state["history_rows"].append(stats)
            ctx.checkpoint()
            write_report(ctx)
        if args.tune_weights:
            tune_weights(ctx)
            write_report(ctx)
    except TimeUp:
        log("時間切れ — 後片付けします")
        if args.tune_weights:
            try:
                ctx.deadline = None  # チューニングは短いので完走させる
                tune_weights(ctx)
            except Exception as e:
                log(f"重みチューニング失敗: {e}")
    except KeyboardInterrupt:
        if ctx.server is None:
            log("中断 — physq eval の起動待ち中でした（サイクル未実行のため保存する状態はありません）")
        else:
            log("中断 — 状態は保存済み。同じコマンドで再開できます")
    finally:
        ctx.checkpoint()
        write_report(ctx)
        if ctx.server:
            ctx.server.close()
        log(f"レポート: {ctx.report_path}")
        log(f"LLM 呼び出し {ctx.llm.calls} 回（失敗 {ctx.llm.failures} 回）")

    if args.apply:
        apply_to_repo(ctx)


if __name__ == "__main__":
    main()
