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
"""self_improve.py の本番 LLM を選ぶための比較スクリプト。

`lms ls` でディスク上にある LLM を全部拾い、1体ずつ:

    lms load <model> → 1問について言い換え5個を生成（本番と同じプロンプト） → lms unload

を繰り返し、ロード時間・生成時間・トークン数・生成結果を並べて出力する。
どこかのモデルで load/生成が失敗・ハング（タイムアウト）しても、そのモデルを
「失敗」として記録して次のモデルへ進む — 1台のモデルの不調で比較全体を
止めない。生成プロンプトは `self_improve.py` の `gen_paraphrases` をそのまま
呼ぶので、ここで良かったモデルはそのまま `--model` に指定して使える。

使い方:
  python3 scripts/model_bakeoff.py                       # 既定の設問で全モデル比較
  python3 scripts/model_bakeoff.py --id <record id>      # 別レコードで比較
  python3 scripts/model_bakeoff.py --question "..."      # 任意の設問で比較（コーパス外）
  python3 scripts/model_bakeoff.py --models qwen/qwen3-1.7b,qwen/qwen3-4b-2507
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import self_improve as si  # noqa: E402  (再利用: LMStudio, gen_paraphrases, prompt)

# 電磁誘導は「同じ意味を保ったまま言い換える」の難易度がちょうどよい定番設問。
DEFAULT_RECORD_ID = "3acc4b9d-f325-4d22-92e0-bb93ad050ad5"

LOAD_TIMEOUT_S = 240  # 1モデルのロードに割く上限（ここで固まったら失敗扱いで次へ）
UNLOAD_TIMEOUT_S = 30


def log(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def list_downloaded_llms() -> list[dict]:
    r = subprocess.run(
        ["lms", "ls", "--llm", "--json"], capture_output=True, text=True, timeout=30
    )
    if r.returncode != 0:
        sys.exit(f"`lms ls` に失敗しました: {r.stderr.strip()}")
    return json.loads(r.stdout)


def safe_identifier(model_key: str) -> str:
    return "bakeoff-" + model_key.replace("/", "-").replace("@", "-")


def lms_load(model_key: str, identifier: str, context_length: int | None) -> tuple[bool, str]:
    cmd = ["lms", "load", model_key, "--identifier", identifier, "-y"]
    if context_length:
        cmd += ["-c", str(context_length)]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=LOAD_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        return False, f"lms load がタイムアウトしました（{LOAD_TIMEOUT_S}秒）"
    if r.returncode != 0:
        return False, (r.stderr or r.stdout).strip()[:500]
    return True, ""


def lms_unload(identifier: str) -> None:
    try:
        subprocess.run(
            ["lms", "unload", identifier],
            capture_output=True, text=True, timeout=UNLOAD_TIMEOUT_S,
        )
    except subprocess.TimeoutExpired:
        log(f"  警告: {identifier} の unload がタイムアウトしました（手動確認推奨: lms ps）")


def chat_with_usage(
    server: str, model_ident: str, system: str, user: str,
    temperature: float, max_tokens: int, timeout: float,
    reasoning_effort: str | None = "none",
) -> tuple[str, dict | None]:
    """si.LMStudio.chat と同じリクエストだが、比較用にトークン使用量も返す。

    reasoning_effort="none" が既定: 実測で thinking モデルは簡単なタスクにも
    数百トークンの reasoning を使い、max_tokens を使い切って本文が空になる
    ことがある（Qwen3.5-9B で確認、432秒かけて空応答）。この比較スクリプトの
    目的は「本番タスクにかかる実時間・トークン数」を測ることなので、reasoning
    は既定でオフにする。
    """
    payload = {
        "model": model_ident,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": False,
    }
    if reasoning_effort is not None:
        payload["reasoning_effort"] = reasoning_effort
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(
        f"{server.rstrip('/')}/v1/chat/completions",
        data=body,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = json.load(resp)
    return data["choices"][0]["message"]["content"], data.get("usage")


def find_record(record_id: str) -> dict | None:
    data = json.loads((REPO / "q_and_a_data.json").read_text(encoding="utf-8"))
    for r in data:
        if r.get("id") == record_id:
            return r
    return None


def run_one_model(
    model: dict, rec: dict, n: int, temp: float, server: str,
    timeout: float, max_tokens: int, context_length: int | None,
    reasoning_effort: str | None,
) -> dict:
    key = model["modelKey"]
    ident = safe_identifier(key)
    result = {
        "model": key,
        "quantization": (model.get("quantization") or {}).get("name"),
        "size_bytes": model.get("sizeBytes"),
        "ok": False,
        "error": None,
        "load_s": None,
        "gen_s": None,
        "prompt_tokens": None,
        "completion_tokens": None,
        "paraphrases": [],
    }

    log(f"=== {key} ===")
    log("  load 中…")
    t0 = time.monotonic()
    ok, err = lms_load(key, ident, context_length)
    result["load_s"] = round(time.monotonic() - t0, 1)
    if not ok:
        result["error"] = f"load 失敗: {err}"
        log(f"  ✗ load 失敗（{result['load_s']}s）: {err}")
        lms_unload(ident)  # 念のため（部分的にロードされている可能性への保険）
        return result
    log(f"  ✓ load 完了（{result['load_s']}s）")

    try:
        t0 = time.monotonic()
        text, usage = chat_with_usage(
            server, ident, si.SYSTEM, _user_prompt(rec, n),
            temp, max_tokens, timeout, reasoning_effort,
        )
        result["gen_s"] = round(time.monotonic() - t0, 1)
        if usage:
            result["prompt_tokens"] = usage.get("prompt_tokens")
            result["completion_tokens"] = usage.get("completion_tokens")
        arr = si.extract_json_array(text)
        if arr is None:
            result["error"] = f"JSON配列を抽出できませんでした。生出力: {text[:300]!r}"
            log(f"  ✗ 生成失敗（{result['gen_s']}s）: {result['error']}")
        else:
            existing_norms = {si.norm_q(q) for q in rec.get("questions") or []}
            out = []
            for item in arr:
                if isinstance(item, str) and si.valid_paraphrase(
                    item.strip(), existing_norms | {si.norm_q(o) for o in out}
                ):
                    out.append(item.strip())
            result["paraphrases"] = out
            result["ok"] = len(out) > 0
            if not result["ok"]:
                result["error"] = f"有効な言い換えが0件でした。生出力: {text[:300]!r}"
            tok_note = (
                f"、prompt={result['prompt_tokens']} completion={result['completion_tokens']}"
                if usage else ""
            )
            log(f"  ✓ 生成完了（{result['gen_s']}s） {len(out)}/{n} 件有効{tok_note}")
            for p in out:
                log(f"    - {p}")
    except Exception as e:  # noqa: BLE001 — 1モデルの想定外エラーで全体を止めない
        result["error"] = f"{type(e).__name__}: {e}"
        log(f"  ✗ 生成中に例外: {result['error']}")
    finally:
        log("  unload 中…")
        lms_unload(ident)

    return result


def _user_prompt(rec: dict, n: int) -> str:
    # gen_paraphrases は LMStudio.chat を直接呼ぶので、同一プロンプトを組み立てて
    # トークン数を実測する（gen_paraphrases 自体を呼ぶと結果の後処理まで一致するが
    # ここでは生出力そのものも見たいので同じ組み立てロジックをそのまま使う）。
    q0 = (rec.get("questions") or [""])[0]
    desc = (rec.get("description") or "")[:200]
    return f"""高校物理のQ&Aサイトの「元の質問」に対して、同じ答えにたどり着くべき言い換え検索クエリを{n}個作ってください。

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
[]

# 出力形式
JSON文字列配列のみを出力する。他のテキストは一切出力しない。
["言い換え1", "言い換え2", ...]

# 良い出力の例（元の質問が「なぜ夕焼けは赤いのか？」だった場合）
["夕焼けが赤く見える理由", "夕日 赤い なぜ", "夕方の空はどうして赤色になるの？", "空の色 夕方 変わる仕組み"]"""


def write_report(results: list[dict], rec: dict, path: Path) -> None:
    lines = [
        "# LLM モデル比較（scripts/model_bakeoff.py）",
        "",
        f"- 設問: 「{(rec.get('questions') or ['?'])[0]}」 (id: {rec.get('id')})",
        f"- 実行: {time.strftime('%Y-%m-%d %H:%M:%S')}",
        "",
        "| モデル | 量子化 | load秒 | 生成秒 | prompt tok | completion tok | 有効言い換え | 結果 |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for r in results:
        status = "OK" if r["ok"] else f"NG: {r['error']}"
        lines.append(
            f"| {r['model']} | {r['quantization']} | {r['load_s']} | {r['gen_s']} | "
            f"{r['prompt_tokens']} | {r['completion_tokens']} | "
            f"{len(r['paraphrases'])} | {status} |"
        )
    lines.append("")
    for r in results:
        lines.append(f"## {r['model']}")
        lines.append("")
        if r["error"]:
            lines.append(f"エラー: {r['error']}")
        for p in r["paraphrases"]:
            lines.append(f"- {p}")
        lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--id", default=DEFAULT_RECORD_ID, help="比較に使う q_and_a_data.json のレコードID")
    ap.add_argument("--question", help="レコードの代わりに任意の設問文字列を使う（description なし）")
    ap.add_argument("--models", help="modelKey をカンマ区切りで指定（省略時: lms ls --llm の全件）")
    ap.add_argument("--n", type=int, default=5, help="生成する言い換え数")
    ap.add_argument("--temperature", type=float, default=0.9)
    ap.add_argument("--server", default="http://localhost:1234")
    ap.add_argument("--llm-timeout", type=float, default=180, help="1回のchat呼び出しのタイムアウト秒")
    ap.add_argument("--llm-max-tokens", type=int, default=4096)
    ap.add_argument("--reasoning-effort", default="none",
                    help='既定 "none"（実測: thinkingモデルは簡単なタスクにも大量の'
                         'reasoning tokenを使い、9Bモデルで432秒かけて本文が空になる'
                         'ケースを確認。空文字列でフィールド自体を送らない）')
    ap.add_argument("--context-length", type=int, help="lms load -c に渡すコンテキスト長")
    ap.add_argument("--out", default=str(REPO / "self_improve_work" / "model_bakeoff.md"))
    args = ap.parse_args()

    if args.question:
        rec = {"id": "(ad-hoc)", "questions": [args.question], "description": ""}
    else:
        rec = find_record(args.id)
        if rec is None:
            sys.exit(f"id {args.id} が q_and_a_data.json に見つかりません")

    all_models = list_downloaded_llms()
    if args.models:
        wanted = set(args.models.split(","))
        models = [m for m in all_models if m["modelKey"] in wanted]
        missing = wanted - {m["modelKey"] for m in models}
        if missing:
            log(f"警告: ディスク上に見つからないモデルを無視します: {missing}")
    else:
        models = all_models

    if not models:
        sys.exit("比較対象の LLM が見つかりません（`lms ls --llm` を確認してください）")

    log(f"設問: 「{(rec.get('questions') or ['?'])[0]}」")
    log(f"対象モデル: {[m['modelKey'] for m in models]}")

    results = []
    for model in models:
        r = run_one_model(
            model, rec, args.n, args.temperature, args.server,
            args.llm_timeout, args.llm_max_tokens, args.context_length,
            args.reasoning_effort or None,
        )
        results.append(r)

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    write_report(results, rec, out_path)

    log("")
    log("=== まとめ ===")
    for r in results:
        status = "OK" if r["ok"] else f"NG ({r['error']})"
        log(f"{r['model']}: load {r['load_s']}s / 生成 {r['gen_s']}s / "
            f"{len(r['paraphrases'])}件 / {status}")
    log(f"レポート: {out_path}")


if __name__ == "__main__":
    main()
