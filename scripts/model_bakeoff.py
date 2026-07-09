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

1モデルにつき load は1回だけ（設問ごとに load/unload はしない）。設問数が
増えても load 待ちが増えないので、複数設問での品質比較が現実的な時間で回せる。

使い方:
  python3 scripts/model_bakeoff.py                       # 既定の1問で全モデル比較
  python3 scripts/model_bakeoff.py --diverse              # 質問タイプの異なる既定6問で比較
  python3 scripts/model_bakeoff.py --ids id1,id2,id3      # 任意の複数レコードで比較
  python3 scripts/model_bakeoff.py --id <record id>       # 単一レコードで比較
  python3 scripts/model_bakeoff.py --question "..."       # 任意の設問で比較（コーパス外）
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

# --diverse: 設問の「型」が異なる6問（現象説明／抽象概念の定義／数式の証明／
# 英訳／単純な定義／法則名）。1つの型だけで判定すると特定モデルの得意分野に
# 引っ張られるため、質問タイプを散らして比較する。
DIVERSE_RECORD_IDS = [
    "3acc4b9d-f325-4d22-92e0-bb93ad050ad5",  # 電磁誘導とは？（現象・仕組み説明）
    "ddb5a799-014e-48ba-b14c-bf04fe41ff5c",  # 電位とは何か？（抽象概念の定義）
    "bd0dd5b0-938d-4c1c-9ea8-05b0cedf4e16",  # 積分の面積公式の証明（数式・なぜ型）
    "4193b42a-f1f2-4a0e-924f-2f9387228d1f",  # 力は英語でなんという？（訳語・一問一答）
    "af39beee-99ae-4eec-9ebf-f563296f08d2",  # 電流とは？（単純な定義、最頻カテゴリ）
    "50c918b8-d269-4333-907f-23bcc4581a29",  # オームの法則とは？（法則名の定義）
]

# propose_terms のテスト用「ダミーの1位競合レコード」。全テスト設問と無関係な
# トピックにしておくことで、ルール2「競合レコードの内容を指す語は禁止」の
# 遵守具合を見やすくする。実際の物理eval順位ではなく固定値なので注意。
FAKE_COMPETITOR_ID = "489b551b-15df-4700-87a7-44b01257d0b0"  # 力学的エネルギー保存則とは？

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


def find_records(record_ids: list[str]) -> list[dict]:
    data = json.loads((REPO / "q_and_a_data.json").read_text(encoding="utf-8"))
    by_id = {r.get("id"): r for r in data}
    out = []
    for rid in record_ids:
        rec = by_id.get(rid)
        if rec is None:
            sys.exit(f"id {rid} が q_and_a_data.json に見つかりません")
        out.append(rec)
    return out


def run_one_question(
    ident: str, rec: dict, n: int, temp: float, server: str,
    timeout: float, max_tokens: int, reasoning_effort: str | None,
) -> dict:
    """1モデル（ロード済み）に1問だけ投げる。load/unload はしない。"""
    q0 = (rec.get("questions") or ["?"])[0]
    qresult = {
        "id": rec.get("id"), "question": q0, "ok": False, "error": None,
        "gen_s": None, "prompt_tokens": None, "completion_tokens": None,
        "paraphrases": [],
    }
    try:
        t0 = time.monotonic()
        text, usage = chat_with_usage(
            server, ident, si.SYSTEM, _user_prompt(rec, n),
            temp, max_tokens, timeout, reasoning_effort,
        )
        qresult["gen_s"] = round(time.monotonic() - t0, 1)
        if usage:
            qresult["prompt_tokens"] = usage.get("prompt_tokens")
            qresult["completion_tokens"] = usage.get("completion_tokens")
        arr = si.extract_json_array(text)
        if arr is None:
            qresult["error"] = f"JSON配列を抽出できませんでした。生出力: {text[:300]!r}"
            log(f"    ✗ 「{q0}」生成失敗（{qresult['gen_s']}s）: {qresult['error']}")
        else:
            existing_norms = {si.norm_q(q) for q in rec.get("questions") or []}
            out = []
            for item in arr:
                if isinstance(item, str) and si.valid_paraphrase(
                    item.strip(), existing_norms | {si.norm_q(o) for o in out}
                ):
                    out.append(item.strip())
            qresult["paraphrases"] = out
            qresult["ok"] = len(out) > 0
            if not qresult["ok"]:
                qresult["error"] = f"有効な言い換えが0件でした。生出力: {text[:300]!r}"
            log(f"    ✓ 「{q0}」（{qresult['gen_s']}s） {len(out)}/{n} 件有効")
            for p in out:
                log(f"      - {p}")
    except Exception as e:  # noqa: BLE001 — 1問の失敗で他の設問/モデルを止めない
        qresult["error"] = f"{type(e).__name__}: {e}"
        log(f"    ✗ 「{q0}」生成中に例外: {qresult['error']}")
    return qresult


def _terms_user_prompt(rec: dict, fails: list[dict], by_id: dict) -> str:
    # self_improve.py の propose_terms と同一プロンプト（比較用に生出力も見たい
    # ので直接呼ばず複製 — 本体を変更したら双方直すこと）。
    q0 = (rec.get("questions") or [""])[0]
    desc = (rec.get("description") or "")[:300]
    lines = []
    for f in fails[:5]:
        rank = f["rank"]
        top1_q = (by_id.get(f["top1_id"], {}).get("questions") or ["?"])[0]
        lines.append(f'- 「{f["query"]}」 → 正解は現在{rank if rank else "圏外"}位。'
                     f'かわりに1位だったのは「{top1_q[:60]}」')
    kw = json.dumps(rec.get("keywords") or [], ensure_ascii=False)
    syn = json.dumps(rec.get("synonyms") or [], ensure_ascii=False)
    return f"""高校物理のQ&A検索システムで、下のクエリに対して本来1位に出るべきレコード（正解レコード）が1位になれませんでした。
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


def run_one_term_question(
    ident: str, rec: dict, paraphrases: list[str], by_id: dict, all_records: list[dict],
    server: str, timeout: float, max_tokens: int, reasoning_effort: str | None,
) -> dict:
    """propose_terms と同じプロンプト・同じフィルタ（valid_term + is_too_generic）
    を使い、そのモデル自身が今しがた生成した言い換えを「失敗クエリ」に見立てて
    keywords/synonyms 提案をテストする。rank/競合レコードはダミー値（実測ではない）。
    """
    q0 = (rec.get("questions") or ["?"])[0]
    tresult = {
        "gen_s": None, "prompt_tokens": None, "completion_tokens": None,
        "accepted": {"keywords": [], "synonyms": []},
        "rejected_generic": [],  # is_too_generic に弾かれた候補（ガードが効いた語）
        "error": None,
    }
    if not paraphrases:
        return tresult
    fails = [
        {"query": q, "rank": 5, "top1_id": FAKE_COMPETITOR_ID}
        for q in paraphrases[:3]
    ]
    try:
        t0 = time.monotonic()
        text, usage = chat_with_usage(
            server, ident, si.SYSTEM, _terms_user_prompt(rec, fails, by_id),
            0.3, max_tokens, timeout, reasoning_effort,
        )
        tresult["gen_s"] = round(time.monotonic() - t0, 1)
        if usage:
            tresult["prompt_tokens"] = usage.get("prompt_tokens")
            tresult["completion_tokens"] = usage.get("completion_tokens")
        obj = si.extract_json_object(text) or {}
        existing = {k.lower() for k in (rec.get("keywords") or [])}
        existing |= {s.lower() for s in (rec.get("synonyms") or [])}
        for field in ("keywords", "synonyms"):
            vals = obj.get(field)
            if not isinstance(vals, list):
                continue
            for v in vals:
                if not isinstance(v, str):
                    continue
                term = v.strip()
                if not si.valid_term(term, existing):
                    continue
                if si.is_too_generic(term, all_records):
                    tresult["rejected_generic"].append(term)
                    continue
                tresult["accepted"][field].append(term)
                existing.add(term.lower())
        log(f"    keywords/synonyms 提案（{tresult['gen_s']}s）: "
            f"採用 {tresult['accepted']}"
            + (f" / 却下(汎用語) {tresult['rejected_generic']}" if tresult["rejected_generic"] else ""))
    except Exception as e:  # noqa: BLE001
        tresult["error"] = f"{type(e).__name__}: {e}"
        log(f"    ✗ 「{q0}」term提案 中に例外: {tresult['error']}")
    return tresult


def run_one_model(
    model: dict, records: list[dict], n: int, temp: float, server: str,
    timeout: float, max_tokens: int, context_length: int | None,
    reasoning_effort: str | None, test_terms: bool, by_id: dict, all_records: list[dict],
) -> dict:
    key = model["modelKey"]
    ident = safe_identifier(key)
    result = {
        "model": key,
        "quantization": (model.get("quantization") or {}).get("name"),
        "size_bytes": model.get("sizeBytes"),
        "load_s": None,
        "load_error": None,
        "questions": [],
    }

    log(f"=== {key} ===")
    log("  load 中…")
    t0 = time.monotonic()
    ok, err = lms_load(key, ident, context_length)
    result["load_s"] = round(time.monotonic() - t0, 1)
    if not ok:
        result["load_error"] = err
        log(f"  ✗ load 失敗（{result['load_s']}s）: {err}")
        lms_unload(ident)  # 念のため（部分的にロードされている可能性への保険）
        return result
    log(f"  ✓ load 完了（{result['load_s']}s） — {len(records)}問を投げます")

    for rec in records:
        qresult = run_one_question(ident, rec, n, temp, server, timeout, max_tokens, reasoning_effort)
        if test_terms and qresult["ok"]:
            qresult["terms"] = run_one_term_question(
                ident, rec, qresult["paraphrases"], by_id, all_records,
                server, timeout, max_tokens, reasoning_effort,
            )
        result["questions"].append(qresult)

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


def write_report(results: list[dict], records: list[dict], path: Path) -> None:
    lines = [
        "# LLM モデル比較（scripts/model_bakeoff.py）",
        "",
        f"- 設問数: {len(records)}",
    ]
    for rec in records:
        lines.append(f"  - {(rec.get('questions') or ['?'])[0]} (id: {rec.get('id')})")
    lines += [
        f"- 実行: {time.strftime('%Y-%m-%d %H:%M:%S')}",
        "",
        "## サマリ（設問平均）",
        "",
        "| モデル | 量子化 | load秒 | 平均生成秒 | 合計生成秒 | 有効/全設問 |",
        "|---|---|---|---|---|---|",
    ]
    for r in results:
        qs = r["questions"]
        if r["load_error"]:
            lines.append(f"| {r['model']} | {r['quantization']} | {r['load_s']} | - | - | "
                         f"load失敗: {r['load_error']} |")
            continue
        gen_times = [q["gen_s"] for q in qs if q["gen_s"] is not None]
        avg_gen = round(sum(gen_times) / len(gen_times), 1) if gen_times else None
        total_gen = round(sum(gen_times), 1) if gen_times else None
        ok_count = sum(1 for q in qs if q["ok"])
        lines.append(
            f"| {r['model']} | {r['quantization']} | {r['load_s']} | {avg_gen} | "
            f"{total_gen} | {ok_count}/{len(qs)} |"
        )
    lines.append("")

    lines.append("## 設問別・生成秒")
    lines.append("")
    header = "| 設問 | " + " | ".join(r["model"] for r in results) + " |"
    lines.append(header)
    lines.append("|---|" + "---|" * len(results))
    for i, rec in enumerate(records):
        q0 = (rec.get("questions") or ["?"])[0]
        row = [q0[:24]]
        for r in results:
            if r["load_error"] or i >= len(r["questions"]):
                row.append("-")
            else:
                q = r["questions"][i]
                row.append(f"{q['gen_s']}s" if q["ok"] else f"NG({q['gen_s']}s)")
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")

    # keywords/synonyms 提案のサマリ（テストしていれば）
    any_terms = any(q.get("terms") for r in results for q in r["questions"])
    if any_terms:
        lines.append("## keywords/synonyms 提案サマリ（自分の言い換えを失敗クエリに見立てたテスト）")
        lines.append("")
        lines.append("| モデル | 採用keywords | 採用synonyms | 汎用語ガードで却下 |")
        lines.append("|---|---|---|---|")
        for r in results:
            if r["load_error"]:
                continue
            kw_all, syn_all, rej_all = [], [], []
            for q in r["questions"]:
                t = q.get("terms")
                if not t:
                    continue
                kw_all += t["accepted"]["keywords"]
                syn_all += t["accepted"]["synonyms"]
                rej_all += t["rejected_generic"]
            lines.append(f"| {r['model']} | {len(kw_all)} | {len(syn_all)} | {len(rej_all)}"
                         f"{f' {rej_all}' if rej_all else ''} |")
        lines.append("")

    for r in results:
        lines.append(f"## {r['model']}")
        lines.append("")
        if r["load_error"]:
            lines.append(f"load失敗: {r['load_error']}")
            lines.append("")
            continue
        for q in r["questions"]:
            lines.append(f"### 「{q['question']}」")
            if q["error"] and not q["ok"]:
                lines.append(f"エラー: {q['error']}")
            for p in q["paraphrases"]:
                lines.append(f"- {p}")
            t = q.get("terms")
            if t:
                lines.append("")
                lines.append(f"keywords/synonyms 提案 ({t['gen_s']}s):")
                lines.append(f"- 採用: {t['accepted']}")
                if t["rejected_generic"]:
                    lines.append(f"- 却下（コーパス頻出＝汎用語ガード）: {t['rejected_generic']}")
                if t["error"]:
                    lines.append(f"- エラー: {t['error']}")
            lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--id", help="比較に使う q_and_a_data.json のレコードID（単一）")
    ap.add_argument("--ids", help="レコードIDをカンマ区切りで複数指定")
    ap.add_argument("--diverse", action="store_true",
                    help="質問タイプの異なる既定6問（DIVERSE_RECORD_IDS）で比較")
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
    ap.add_argument("--no-test-terms", action="store_true",
                    help="keywords/synonyms 提案（propose_terms）のテストをスキップし、"
                         "言い換え生成だけ比較する")
    ap.add_argument("--out", default=str(REPO / "self_improve_work" / "model_bakeoff.md"))
    args = ap.parse_args()

    all_data = json.loads((REPO / "q_and_a_data.json").read_text(encoding="utf-8"))
    by_id = {r.get("id"): r for r in all_data}

    if args.question:
        records = [{"id": "(ad-hoc)", "questions": [args.question], "description": ""}]
    elif args.ids:
        records = find_records(args.ids.split(","))
    elif args.diverse:
        records = find_records(DIVERSE_RECORD_IDS)
    else:
        records = find_records([args.id or DEFAULT_RECORD_ID])

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

    log(f"設問数: {len(records)}: " + " / ".join((r.get('questions') or ['?'])[0] for r in records))
    log(f"対象モデル: {[m['modelKey'] for m in models]}")

    results = []
    for model in models:
        r = run_one_model(
            model, records, args.n, args.temperature, args.server,
            args.llm_timeout, args.llm_max_tokens, args.context_length,
            args.reasoning_effort or None, not args.no_test_terms, by_id, all_data,
        )
        results.append(r)

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    write_report(results, records, out_path)

    log("")
    log("=== まとめ ===")
    for r in results:
        if r["load_error"]:
            log(f"{r['model']}: load 失敗（{r['load_s']}s）: {r['load_error']}")
            continue
        gen_times = [q["gen_s"] for q in r["questions"] if q["gen_s"] is not None]
        avg_gen = round(sum(gen_times) / len(gen_times), 1) if gen_times else None
        ok_count = sum(1 for q in r["questions"] if q["ok"])
        log(f"{r['model']}: load {r['load_s']}s / 平均生成 {avg_gen}s / "
            f"{ok_count}/{len(r['questions'])} 問成功")
    log(f"レポート: {out_path}")


if __name__ == "__main__":
    main()
