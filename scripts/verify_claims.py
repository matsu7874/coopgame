"""ドキュメントに書いた数値を、保存した CSV から再計算して照合する。

使い方 (リポジトリのルートで実行):
  python3 scripts/verify_claims.py
  python3 scripts/verify_claims.py --instances scripts/compare/work   # 配分の検証も行う

--instances には `scripts/compare/make_instances.sh` で作ったディレクトリ (small/, large/) を渡す。
渡すと、coopgame-rs の全配分の Kohlberg 判定と、誤りと判定した他実装の配分の再判定も行う
(target/release/coopgame が必要。`cargo build --release --features cli` で作る)。

照合する主張ごとに、どのドキュメントの記述かを示す。1 つでも食い違えば終了コード 1 を返す。
"""
import argparse
import collections
import csv
import os
import re
import subprocess
import sys
import tempfile

failures = []


def check(doc, claim, actual, expected):
    ok = actual == expected
    mark = "ok  " if ok else "FAIL"
    print(f"[{mark}] {doc}: {claim}: {actual!r}" + ("" if ok else f" (期待値 {expected!r})"))
    if not ok:
        failures.append((doc, claim))


def read(path):
    with open(path) as f:
        return list(csv.DictReader(f))


# ---------------------------------------------------------------- 既存実装との比較
def allocation(row):
    return [float(v) for v in row["allocation"].split()]


def same(a, b):
    scale = max(1.0, max(abs(v) for v in b))
    return len(a) == len(b) and max(abs(x - y) for x, y in zip(a, b)) <= 1e-6 * scale


def outcome(row, reference):
    if row["status"] != "ok":
        return row["status"].split(":")[0]
    key = (row["method"], row["type"], row["n"], row["seed"])
    return "match" if same(allocation(row), reference[key]) else "wrong"


def verify_comparison():
    doc = "docs/comparison.md"
    small = read("data/compare/small.csv")
    large = read("data/compare/large.csv")
    retry = read("data/compare/tuglab-retry-tol1e-8.csv")
    reference = {}
    for r in small + large:
        if r["impl"] == "coopgame-rs":
            reference[(r["method"], r["type"], r["n"], r["seed"])] = allocation(r)

    check(doc, "small のインスタンス数", len({(r["type"], r["n"], r["seed"]) for r in small}), 126)
    check(doc, "large のインスタンス数", len({(r["type"], r["n"], r["seed"]) for r in large}), 18)
    for impl in ("coopgame-rs", "coopgame-rs-full"):
        rows = [r for r in small + large if r["impl"] == impl]
        check(doc, f"{impl} は全 288 件で制約生成版と一致",
              collections.Counter(outcome(r, reference) for r in rows), collections.Counter({"match": 288}))

    def tally(rows, impl):
        return dict(collections.Counter(outcome(r, reference) for r in rows if r["impl"] == impl))

    check(doc, "CoopGame (small)", tally(small, "CoopGame"), {"match": 252})
    check(doc, "CoopGame (large)", tally(large, "CoopGame"), {"match": 32, "wrong": 4})
    wrong = sorted((r["method"], r["type"], r["n"]) for r in large
                   if r["impl"] == "CoopGame" and outcome(r, reference) == "wrong")
    check(doc, "CoopGame の誤りはタイプ 1 の n = 17, 18 の仁・プレ仁", wrong,
          sorted((m, "1", n) for m in ("nucleolus", "prenucleolus") for n in ("17", "18")))
    check(doc, "TUGLab 既定 (small)", tally(small, "TUGLab"),
          {"match": 226, "wrong": 4, "error": 17, "timeout": 2, "skipped": 3})
    check(doc, "TUGLab 1e-8 (large)", tally(large, "TUGLab"),
          {"match": 26, "wrong": 4, "timeout": 2, "skipped": 4})

    retried = {(r["method"], r["type"], r["n"], r["seed"]): r for r in retry}
    bad = [r for r in small if r["impl"] == "TUGLab" and outcome(r, reference) != "match"]
    after = collections.Counter(outcome(retried[(r["method"], r["type"], r["n"], r["seed"])], reference) for r in bad)
    check(doc, "TUGLab の失敗 26 件を 1e-8 で再実行", (len(bad), dict(after)),
          (26, {"match": 16, "error": 7, "wrong": 1, "timeout": 1, "skipped": 1}))
    tug_errors = collections.Counter(r["type"] for r in small if r["impl"] == "TUGLab" and r["status"].startswith("error"))
    check(doc, "TUGLab のエラーはタイプ 1-4 の全てで起きた", sorted(tug_errors), ["1", "2", "3", "4"])
    tug_wrong = sorted({r["type"] for r in small + large if r["impl"] == "TUGLab" and outcome(r, reference) == "wrong"})
    check(doc, "TUGLab の誤りはタイプ 2, 4", tug_wrong, ["2", "4"])

    def max_time(impl, method, n):
        values = [float(r["seconds"]) for r in large
                  if (r["impl"], r["method"], r["n"], r["status"]) == (impl, method, n, "ok")]
        return max(values)

    def time_range(impl, n="18"):
        values = [float(r["seconds"]) for r in large
                  if (r["impl"], r["n"], r["status"]) == (impl, n, "ok")]
        return round(min(values), 2), round(max(values), 1)

    check(doc, "n = 18 の coopgame-rs の範囲", time_range("coopgame-rs"), (0.16, 0.3))
    check(doc, "n = 18 の CoopGame の範囲", time_range("CoopGame"), (2.8, 8.4))
    check(doc, "n = 18 の TUGLab の範囲", (round(time_range("TUGLab")[0]), round(time_range("TUGLab")[1])), (18, 119))
    ratios = []
    for r in large:
        if r["impl"] == "CoopGame" and r["n"] == "18":
            ours = [x for x in large if x["impl"] == "coopgame-rs" and (x["method"], x["type"], x["n"]) == (r["method"], r["type"], r["n"])][0]
            ratios.append(float(r["seconds"]) / float(ours["seconds"]))
    check(doc, "n = 18 の CoopGame / coopgame-rs の比 (四捨五入)", (round(min(ratios)), round(max(ratios))), (13, 50))
    check(doc, "表 n = 18 の仁 (制約生成, 全行, CoopGame, TUGLab)",
          tuple(round(max_time(i, "nucleolus", "18"), 2) for i in ("coopgame-rs", "coopgame-rs-full", "CoopGame", "TUGLab")),
          (0.22, 140.18, 7.9, 118.89))


def coopgame(*args):
    return subprocess.run(["target/release/coopgame", *args], capture_output=True, text=True).stdout


def kohlberg(row, instances):
    folder = "large" if int(row["n"]) >= 13 else "small"
    game = os.path.join(instances, folder, f"t{row['type']}_n{row['n']}_s{row['seed']}.txt")
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as sol:
        sol.write("\n".join(row["allocation"].split()) + "\n")
    try:
        args = ["verify", game, sol.name] + (["--pre"] if row["method"] == "prenucleolus" else [])
        return "kohlberg=true" in coopgame(*args)
    finally:
        os.unlink(sol.name)


def sorted_excesses(game_path, x):
    values = [0.0] + [float(line) for line in open(game_path) if line.strip()]
    full = len(values) - 1
    sums = [0.0] * (full + 1)
    excesses = []
    for mask in range(1, full):
        lowest = (mask & -mask).bit_length() - 1
        sums[mask] = sums[mask & (mask - 1)] + x[lowest]
        excesses.append(values[mask] - sums[mask])
    return sorted(excesses, reverse=True)


def verify_allocations(instances):
    doc = "docs/comparison.md"
    small = read("data/compare/small.csv")
    large = read("data/compare/large.csv")
    ours = [r for r in small + large if r["impl"].startswith("coopgame-rs")]
    check(doc, "coopgame-rs の全 576 配分が Kohlberg 基準を満たす",
          sum(kohlberg(r, instances) for r in ours), len(ours))
    reference = {(r["method"], r["type"], r["n"], r["seed"]): r for r in large if r["impl"] == "coopgame-rs"}
    for r in large:
        if r["impl"] != "CoopGame" or r["status"] != "ok":
            continue
        ref = reference[(r["method"], r["type"], r["n"], r["seed"])]
        if same(allocation(r), allocation(ref)):
            continue
        path = os.path.join(instances, "large", f"t{r['type']}_n{r['n']}_s{r['seed']}.txt")
        theirs, mine = sorted_excesses(path, allocation(r)), sorted_excesses(path, allocation(ref))
        rank = next(i for i, (a, b) in enumerate(zip(theirs, mine)) if abs(a - b) > 1e-7 * max(1.0, abs(b)))
        check(doc, f"CoopGame {r['method']} t{r['type']} n={r['n']} は辞書式に大きい (最初に食い違う順位, 相手の超過の方が大きい, 相手が Kohlberg 基準を満たす)",
              (rank + 1, theirs[rank] > mine[rank], kohlberg(r, instances)), (rank + 1, True, False))


def verify_phase2_bench():
    doc = "docs/performance.md (明示ベクトルの計測例)"
    rows = []
    for name in ("results-n4-12.csv", "results-n13-16.csv", "results-n17-22.csv"):
        rows += read(os.path.join("data", "bench", name))
    check(doc, "ケース数", len(rows), 540)
    check(doc, "全ケースで検証に成功", {(r["error"], r["verified"]) for r in rows}, {("", "true")})


def verify_certified():
    doc = "docs/comparison.md (厳密な検証)"
    rows = read("data/compare/certified.csv")
    check(doc, "検証した配分の数", len(rows), 1124)
    tally = collections.Counter((r["impl"], r["certified"]) for r in rows)
    check(doc, "合否 (coopgame-rs 2 方式・CoopGame・TUGLab)",
          (tally[("coopgame-rs", "true")], tally[("coopgame-rs-full", "true")],
           tally[("CoopGame", "true")], tally[("CoopGame", "false")],
           tally[("TUGLab", "true")], tally[("TUGLab", "false")]),
          (288, 288, 284, 4, 252, 8))
    ours = max(float(r["distance_to_certified"]) for r in rows if r["impl"].startswith("coopgame-rs"))
    passed = max(float(r["distance_to_certified"]) for r in rows
                 if not r["impl"].startswith("coopgame-rs") and r["certified"] == "true")
    failed = min(float(r["distance_to_certified"]) for r in rows if r["certified"] == "false")
    check(doc, "x* からの距離 (自分の最大 <= 3e-11、他の合格の最大 <= 1e-6、不合格の最小 >= 0.02)",
          (ours <= 3e-11, passed <= 1e-6, failed >= 0.02), (True, True, True))
    small = read("data/compare/small.csv")
    large = read("data/compare/large.csv")
    reference = {(r["method"], r["type"], r["n"], r["seed"]): allocation(r)
                 for r in small + large if r["impl"] == "coopgame-rs"}
    wrong = {(r["impl"], r["method"], r["type"], r["n"], r["seed"]) for r in small + large
             if r["impl"] in ("CoopGame", "TUGLab") and outcome(r, reference) == "wrong"}
    rejected = {(r["impl"], r["method"], r["type"], r["n"], r["seed"]) for r in rows if r["certified"] == "false"}
    check(doc, "浮動小数点で誤りとした配分と、有理数で不合格の配分が一致", (len(wrong), wrong == rejected), (12, True))


def verify_oracle_bench():
    doc = "docs/performance.md (オラクルの計測)"
    rows = read("data/bench/oracle-scaling.csv")
    check(doc, "全ての計算に成功", all(not r["max_error"].startswith("error") for r in rows), True)

    def worst(kind, n, column):
        return max(float(r[column]) for r in rows if (r["kind"], r["n"]) == (kind, n))

    def error(kind, n):
        return float(f"{worst(kind, n, 'max_error'):.1e}")

    check(doc, "破産ゲームのタルムード則との差 (請求 1-100 の 40, 70, 100、請求 1-1000 の 40, 70, 100)",
          tuple(error(kind, n) for kind in ("bankruptcy", "bankruptcy-wide") for n in ("40", "70", "100")),
          (1.1e-12, 4.9e-12, 7.3e-12, 1.2e-11, 4.7e-11, 1.0e-10))
    check(doc, "多数決ゲームの等分との差が全て 1.3e-15 以下",
          max(float(r["max_error"]) for r in rows if r["kind"] == "majority") <= 1.3e-15, True)
    check(doc, "破産ゲームは行を追加しない",
          {r["rows_added"] for r in rows if r["kind"].startswith("bankruptcy")}, {"0"})
    check(doc, "seed 数 (請求 1-100 の 100、請求 1-1000 の 100、投票 25)",
          tuple(sum((r["kind"], r["n"]) == key for r in rows)
                for key in (("bankruptcy", "100"), ("bankruptcy-wide", "100"), ("voting", "25"))),
          (3, 10, 2))
    check(doc, "表の最大時間 (請求 1-100 の 40, 70, 100、請求 1-1000 の 40, 70, 100、多数決 21、投票 25)",
          (round(worst("bankruptcy", "40", "seconds"), 2), round(worst("bankruptcy", "70", "seconds"), 1),
           round(worst("bankruptcy", "100", "seconds"), 1), round(worst("bankruptcy-wide", "40", "seconds"), 2),
           round(worst("bankruptcy-wide", "70", "seconds"), 1), round(worst("bankruptcy-wide", "100", "seconds"), 1),
           round(worst("majority", "21", "seconds"), 2), round(worst("voting", "25", "seconds"), 2)),
          (0.30, 12.6, 8.1, 0.74, 17.4, 148.8, 0.06, 0.20))
    check(doc, "表の段の数 (請求 1-100 の 40, 70, 100、請求 1-1000 の 40, 70, 100)",
          tuple(int(worst(kind, n, "levels")) for kind in ("bankruptcy", "bankruptcy-wide")
                for n in ("40", "70", "100")),
          (24, 37, 21, 26, 54, 68))
    check(doc, "表の追加行数 (多数決 21、投票 25)",
          (int(worst("majority", "21", "rows_added")), int(worst("voting", "25", "rows_added"))), (336, 400))
    wide = sorted(float(r["seconds"]) for r in rows if (r["kind"], r["n"]) == ("bankruptcy-wide", "100"))
    check(doc, "請求 1-1000 の 100 人で 23 秒以下の seed の数", sum(t <= 23 for t in wide), 9)


def verify_voting_bench():
    """拒否権のある重み付き投票ゲームの、修正前後の時間と最大メモリ、列挙の上限の根拠。"""
    doc = "docs/performance.md (拒否権のある重み付き投票ゲーム)"
    before = {(r["kind"], int(r["n"])): r for r in read("data/bench/voting-oracle-before.csv")}
    after = {(r["kind"], int(r["n"])): r for r in read("data/bench/voting-oracle-after.csv")}

    def cell(rows, kind, n):
        r = rows[(kind, n)]
        if r["status"] != "ok":
            return r["status"]
        return (round(float(r["seconds"]), 2), int(r["max_rss_mb"]))

    check(doc, "修正後は全ての計測が完了した", {r["status"] for r in after.values()}, {"ok"})
    check(doc, "修正前は 26 人で 6 GB を超えて止まった (拒否権 3 人, 2 人)",
          (cell(before, "veto3", 26), cell(before, "veto2", 26)), ("rss_limit", "rss_limit"))
    table = {
        ("veto3", 20): ((1.44, 668), (0.07, 10)),
        ("veto3", 24): ((26.67, 3718), (0.10, 13)),
        ("veto3", 26): ("rss_limit", (0.12, 14)),
        ("veto3", 40): ("skipped", (0.43, 25)),
        ("veto3", 60): ("skipped", (1.41, 36)),
        ("veto2", 24): ((20.68, 3824), (0.07, 11)),
        ("veto2", 50): ("skipped", (0.58, 23)),
        ("majority", 31): ((0.23, 9), (0.23, 10)),
        ("random100", 30): ((2.49, 91), (2.50, 92)),
        ("random100q90", 15): ((0.16, 13), (0.13, 12)),
    }
    for (kind, n), value in table.items():
        check(doc, f"{kind} の {n} 人 (修正前, 修正後)", (cell(before, kind, n), cell(after, kind, n)), value)
    veto = [r for (kind, _), r in after.items() if kind.startswith("veto")]
    check(doc, "拒否権のあるゲームの仁との差が全て 5.6e-17 以下",
          max(float(r["max_error"]) for r in veto) <= 5.6e-17, True)
    others = [key for key in before if not key[0].startswith("veto")]
    check(doc, "拒否権のあるゲーム以外の組の数", len(others), 30)
    check(doc, "拒否権のあるゲーム以外で、LP を解いた回数と追加した行が同じ",
          all((before[k]["lp_solves"], before[k]["rows_added"]) == (after[k]["lp_solves"], after[k]["rows_added"])
              for k in others), True)
    check(doc, "拒否権のあるゲーム以外で、時間の差が修正前の 2% と 0.04 秒の大きい方以内、最大メモリの増加が 5 MB 以下",
          (all(abs(float(after[k]["seconds"]) - float(before[k]["seconds"]))
               <= max(0.02 * float(before[k]["seconds"]), 0.04) for k in others),
           max(int(after[k]["max_rss_mb"]) - int(before[k]["max_rss_mb"]) for k in others) <= 5), (True, True))

    budget = read("data/bench/voting-budget.csv")
    with open("src/games/voting.rs", encoding="utf-8") as f:
        constant = int(re.search(r"const ENUMERATION_BUDGET: usize = (\d+);", f.read()).group(1))
    check(doc, "列挙の上限 ENUMERATION_BUDGET", constant, 256)

    def row(b, kind, n):
        return next(r for r in budget if (int(r["budget"]), r["kind"], int(r["n"])) == (b, kind, n))

    one_stage = ("majority", "apex", "random10", "random100", "random100q23")
    check(doc, "仁が 1 段で決まるゲームでは、確定済みの提携のある探索で読んだ提携が全て 0 個",
          {r["max_read"] for r in budget if r["kind"] in one_stage}, {"0"})
    check(doc, "重み 1-100・基準 9/10 の 10 人は最大 73 個を読み、上限 64 では上限まで読む",
          (row(256, "random100q90", 10)["max_read"], int(row(64, "random100q90", 10)["searches_over_budget"]) > 0),
          ("73", True))
    check(doc, "拒否権 3 人の 40 人 (上限 256, 4096 の秒と MB)",
          [(round(float(row(b, "veto3", 40)["seconds"]), 2), int(row(b, "veto3", 40)["max_rss_mb"])) for b in (256, 4096)],
          [(0.42, 24), (2.37, 377)])
    with open("src/games/voting.rs", encoding="utf-8") as f:
        source = f.read()
    check(doc, "定数の説明の数値 (上限 256 で 0.42 秒・24 MB、1024 で 0.85 秒・69 MB、4096 で 2.4 秒・377 MB)",
          "上限 256 で 0.42 秒・24 MB、1024 で 0.85 秒・69 MB、" in source and "4096 で 2.4 秒・377 MB" in source
          and (round(float(row(1024, "veto3", 40)["seconds"]), 2), int(row(1024, "veto3", 40)["max_rss_mb"])) == (0.85, 69),
          True)


def verify_oracle_tolerance():
    doc = "docs/performance.md (上限と既定値の根拠)"
    rows = read("data/bench/oracle-tolerance.csv")
    check(doc, "40 人の投票ゲームを解いた比", [r["ratio"] for r in rows], ["1e-7", "1e-8", "1e-9"])
    check(doc, "40 人の投票ゲーム (全て解けた, 最大の秒, 追加した行, 仁との差の最大)",
          (all(not r["max_error"].startswith("error") for r in rows),
           round(max(float(r["seconds"]) for r in rows), 1),
           {int(r["rows_added"]) for r in rows},
           float(f"{max(float(r['max_error']) for r in rows):.1e}")),
          (True, 0.2, {160}, 1.9e-15))


def verify_uncertainty_bench():
    doc = "docs/performance.md (値の不確かさの分析の時間)"
    rows = read("data/bench/uncertainty.csv")
    # (分析, ゲーム, 区間の幅) -> 人数 -> mode -> 行
    pairs = collections.defaultdict(lambda: collections.defaultdict(dict))
    for r in rows:
        pairs[(r["analysis"], r["family"], r["relative"])][int(r["players"])][r["mode"]] = r

    def seconds(key, mode, digits):
        return [round(float(by_mode[mode]["seconds"]), digits) for _, by_mode in sorted(pairs[key].items())]

    check(doc, "人数", {key: sorted(by_players) for key, by_players in pairs.items()},
          {("uncertainty", "bnf1", "1e-1"): [5, 8, 10, 12], ("influence", "bnf1", ""): [5, 8, 10, 12],
           ("uncertainty", "integer-1e9", "1e-1"): [4, 6, 8], ("influence", "integer-1e9", ""): [4, 6],
           ("uncertainty", "integer-1e9", "1e-12"): [4, 6]})
    check(doc, "uncertainty (±10%) BNF の秒 (1 回だけ解く, 既定)",
          (seconds(("uncertainty", "bnf1", "1e-1"), "fixed", 2), seconds(("uncertainty", "bnf1", "1e-1"), "auto", 2)),
          ([0.04, 0.40, 0.93, 2.23], [0.04, 0.41, 0.95, 2.29]))
    check(doc, "uncertainty (±10%) 整数の秒 (1 回だけ解く, 既定)",
          (seconds(("uncertainty", "integer-1e9", "1e-1"), "fixed", 2), seconds(("uncertainty", "integer-1e9", "1e-1"), "auto", 2)),
          ([0.02, 0.14, 0.46], [0.02, 0.14, 0.46]))
    check(doc, "influence BNF の秒 (1 回だけ解く, 既定)",
          ([float(f"{v:.2g}") for v in seconds(("influence", "bnf1", ""), "fixed", 3)],
           [float(f"{v:.2g}") for v in seconds(("influence", "bnf1", ""), "auto", 3)]),
          ([0.002, 0.034, 0.12, 0.35], [0.002, 0.033, 0.12, 0.35]))
    key = ("influence", "integer-1e9", "")
    check(doc, "influence 整数の秒 (1 回だけ解く, 既定) と (解き直した, 有理数, 解いた回数)",
          (seconds(key, "fixed", 3), [float(f"{v:.3g}") for v in seconds(key, "auto", 3)],
           [(int(m["auto"]["refined"]), int(m["auto"]["rational"]), int(m["auto"]["solves"])) for _, m in sorted(pairs[key].items())]),
          ([0.003, 0.098], [0.094, 11.4], [(19, 14, 31), (90, 68, 127)]))
    key = ("uncertainty", "integer-1e9", "1e-12")
    check(doc, "uncertainty (±1e-12 倍) 整数の秒 (1 回だけ解く, 既定) と (解き直した, 有理数)",
          ([float(f"{v:.2g}") for v in seconds(key, "fixed", 3)], [float(f"{v:.3g}") for v in seconds(key, "auto", 3)],
           [(int(m["auto"]["refined"]), int(m["auto"]["rational"])) for _, m in sorted(pairs[key].items())]),
          ([0.015, 0.16], [2.98, 37.2], [(200, 200), (200, 200)]))
    plain = [m for by_players in pairs.values() for m in by_players.values() if m["auto"]["refined"] == "0"]
    check(doc, "解き直しがない行は、既定でも解き直し・有理数・approximate が 0",
          all((m["auto"]["rational"], m["auto"]["approximate"]) == ("0", "0") for m in plain), True)
    ratios = [float(m["auto"]["seconds"]) / float(m["fixed"]["seconds"]) for m in plain]
    check(doc, "解き直しがない行の時間の比 (既定 / 1 回だけ解く) の範囲", (round(min(ratios), 2), round(max(ratios), 2)), (0.97, 1.09))
    slow = [m for by_players in pairs.values() for m in by_players.values() if m["auto"]["refined"] != "0"]
    ratios = [float(m["auto"]["seconds"]) / float(m["fixed"]["seconds"]) for m in slow]
    check(doc, "解き直した行の時間の比の範囲", (round(min(ratios)), round(max(ratios))), (31, 237))


def verify_guarantees():
    doc = "docs/guarantees.md"
    rows = read("data/analysis/assumption-study.csv")
    check(doc, "ゲーム数", len(rows), 420)
    table = {}
    for c in ("convex", "bnf1", "bnf2", "bnf4", "superadditive", "voting"):
        group = [r for r in rows if r["class"] == c]
        tally = collections.Counter(r["outcome"] for r in group)
        table[c] = (len(group), sum(r["convex"] == "true" for r in group),
                    tally["certified"], tally["refuted"], tally["error"])
    check(doc, "クラス別の表 (数, 凸, 合格, 否定, 失敗)", table, {
        "convex": (70, 70, 70, 0, 0), "bnf1": (70, 0, 1, 1, 68), "bnf2": (70, 0, 0, 2, 68),
        "bnf4": (70, 0, 0, 1, 69), "superadditive": (70, 3, 10, 1, 59), "voting": (70, 0, 21, 6, 43)})
    check(doc, "未決は 0 個", sum(r["outcome"] == "undecided" for r in rows), 0)
    check(doc, "凸ゲームは全て合格", {r["outcome"] for r in rows if r["convex"] == "true"}, {"certified"})
    nonconvex = [r for r in rows if r["convex"] == "false"]
    check(doc, "凸でないゲームの内訳 (数, 失敗, 合格, 否定)",
          (len(nonconvex), sum(r["outcome"] == "error" for r in nonconvex),
           sum(r["outcome"] == "certified" for r in nonconvex), sum(r["outcome"] == "refuted" for r in nonconvex)),
          (347, 307, 29, 11))
    check(doc, "失敗の内訳 (numerical, limit)",
          tuple(sum(r["detail"] == d for r in rows if r["outcome"] == "error") for d in ("numerical", "limit")),
          (198, 109))
    check(doc, "凸でないのに合格した n = 4 のゲーム", sum(r["n"] == "4" for r in nonconvex if r["outcome"] == "certified"), 16)
    refuted = collections.Counter((r["detail"], r["in_prekernel"]) for r in rows if r["outcome"] == "refuted")
    check(doc, "否定の内訳 (配分集合の外, プレカーネルだが仁でない, プレカーネルでもない)",
          (refuted[("not_imputation", "true")], refuted[("not_nucleolus", "true")], refuted[("not_nucleolus", "false")]),
          (3, 1, 7))
    check(doc, "合格した結果の LP の仁との差の最大 <= 7.5e-10",
          max(float(r["distance_nucleolus"]) for r in rows if r["outcome"] == "certified") <= 7.5e-10, True)

    graph = read("data/bench/convex-graph.csv")
    bankruptcy = read("data/bench/convex-bankruptcy.csv")

    def worst(rows, n, column):
        return max(float(r[column]) for r in rows if r["n"] == str(n))

    check(doc, "誘導部分グラフの時間 (16, 24, 32, 48, 64)",
          tuple(round(worst(graph, n, "seconds"), 3 if n < 32 else 2 if n < 48 else 1) for n in (16, 24, 32, 48, 64)),
          (0.048, 0.341, 0.72, 4.2, 17.2))
    check(doc, "誘導部分グラフ n <= 16 の LP の仁との差 <= 1e-13",
          max(float(r["max_error"]) for r in graph if r["max_error"]) <= 1e-13, True)
    check(doc, "誘導部分グラフ n = 24 のカーネル条件の反例なし", {r["pair_check"] for r in graph if r["n"] == "24"}, {"no_counterexample"})
    check(doc, "破産の時間・周回・タルムード則との差 (20, 40)",
          (round(worst(bankruptcy, 20, "seconds"), 2), int(worst(bankruptcy, 20, "sweeps")),
           round(worst(bankruptcy, 40, "seconds"), 1), int(worst(bankruptcy, 40, "sweeps")),
           worst(bankruptcy, 40, "max_error") <= 6e-7),
          (0.6, 56, 17.5, 62, True))
    threshold = read("data/bench/auto-threshold.csv")
    lp = {n: worst(threshold, n, "lp_seconds") for n in (16, 18)}
    cv = {n: worst(threshold, n, "convex_seconds") for n in (16, 18)}
    check(doc, "境目: 16 人は LP が速く、18 人は凸ゲームの手法が速い", (lp[16] < cv[16], cv[18] < lp[18]), (True, True))
    check(doc, "閾値の表の差の最大 (n = 20) <= 8e-10", worst(threshold, 20, "relative_difference") <= 8e-10, True)


def verify_exact_solver():
    doc = "docs/performance.md (有理数による厳密な計算)"
    rows = read("data/bench/exact-scaling.csv")
    check(doc, "計測したゲーム数と全て一致", (len(rows), all(r["matches_certified"] == "true" for r in rows)), (24, True))

    def worst(n):
        return max(float(r["seconds"]) for r in rows if r["n"] == str(n))

    check(doc, "最大時間 (n = 8, 10)", (round(worst(8), 1), round(worst(10))), (2.5, 153))


def verify_sampled():
    doc = "docs/performance.md (サンプリングした仁の誤差)"
    rows = read("data/analysis/sampled-convergence.csv")
    check(doc, "収束の行数 (3 タイプ x 2 サイズ x 5 seed x 7 段階)", len(rows), 210)
    group = [r for r in rows if (r["n"], r["pairs"]) == ("10", "1600")]
    share = max(int(r["evaluations"]) for r in group) / (2 ** 10 - 2)
    check(doc, "n = 10、pairs = 1600 で評価した真部分提携の割合 (95% 以上)", share >= 0.95, True)
    worst = max(group, key=lambda r: float(r["relative_error"]))
    check(doc, "その最大の相対誤差 (タイプ 4 の 0.055)", (worst["type"], round(float(worst["relative_error"]), 3)), ("4", 0.055))


def verify_variant_limits():
    """全ての行を LP に入れる解と交渉集合の判定の、プレイヤー数の上限の根拠。"""
    doc = "docs/performance.md (全ての行を LP に入れる解と交渉集合の判定)"
    rows = read("data/bench/variant-limits.csv")
    check(doc, "全ての計測が完了した", {r["status"] for r in rows}, {"ok"})
    constants = {}
    for path in ("src/nucleolus/variants.rs", "src/bargaining.rs"):
        with open(path) as f:
            for name, value in re.findall(r"pub const (MAX_\w+_PLAYERS): usize = (\d+);", f.read()):
                constants[name] = int(value)

    def group(method, n):
        return [r for r in rows if r["method"] == method and int(r["n"]) == n]

    def within(method, n):
        # 1 回が 1 分以内、最大メモリが 1 GB 以内
        return all(float(r["seconds"]) <= 60 and float(r["max_rss_mb"]) <= 1024 for r in group(method, n))

    def worst(method, n):
        g = group(method, n)
        return (round(max(float(r["seconds"]) for r in g), 1), max(int(r["max_rss_mb"]) for r in g))

    limits = [("per-capita", "MAX_PER_CAPITA_PLAYERS"), ("proportional", "MAX_PROPORTIONAL_PLAYERS"),
              ("disruption", "MAX_DISRUPTION_PLAYERS"), ("bargaining", "MAX_BARGAINING_SET_PLAYERS")]
    for method, name in limits:
        largest = max(int(r["n"]) for r in rows if r["method"] == method and within(method, int(r["n"])))
        check(doc, f"{method}: 条件を満たす最大の人数が {name}", largest, constants[name])
        check(doc, f"{method}: 1 人多い人数を測り、条件を超えた", (bool(group(method, largest + 1)), within(method, largest + 1)), (True, False))
    expected = {
        ("per-capita", 17): (19.3, 924), ("per-capita", 18): (52.4, 1383),
        ("proportional", 17): (39.2, 937), ("proportional", 18): (57.0, 1681),
        ("disruption", 16): (7.9, 847), ("disruption", 17): (19.7, 1487),
        ("modiclus", 7): (0.4, 51),
        ("bargaining", 11): (21.8, 7), ("bargaining", 12): (80.6, 10),
    }
    for (method, n), value in expected.items():
        check(doc, f"{method} の {n} 人での最大 (秒, MB)", worst(method, n), value)
    check(doc, "modiclus の上限 7 人で条件を満たす", (constants["MAX_MODICLUS_PLAYERS"], within("modiclus", 7)), (7, True))
    check(doc, "基準 2 の投票ゲームの 11 人の交渉集合の LP の数", [int(r["lp_solves"]) for r in group("bargaining", 11) if r["game"] == "quota2"], [56210])


def verify_tolerance_study():
    doc = "docs/performance.md (仁の許容誤差の決め方)"
    rows = {(r["family"], r["players"]): r for r in read("data/analysis/tolerance-study.csv")}

    def counts(family, players):
        r = rows[(family, players)]
        return tuple(int(r[k]) for k in ("games", "fixed_correct", "auto_correct", "refined", "rational", "approximate"))

    # (ゲーム数, 1 回だけ解く: 正しい数, 既定: 正しい数, 解き直した数, 有理数に切り替えた数, approximate の数)
    check(doc, "整数 3-6 人の表", [counts(f"integer-{s}", "3-6") for s in ("1e0", "1e5", "1e6", "1e7", "1e8", "1e9", "1e12")],
          [(200, 200, 200, 0, 0, 0), (200, 200, 200, 0, 0, 0), (200, 166, 200, 34, 0, 0), (200, 84, 200, 116, 0, 0),
           (200, 91, 200, 109, 1, 0), (200, 79, 200, 121, 50, 0), (200, 50, 120, 70, 70, 0)])
    check(doc, "整数 (下限) 3-6 人の表", [counts(f"lower-{s}", "3-6") for s in ("1e7", "1e9", "1e10")],
          [(200, 116, 200, 84, 0, 0), (200, 112, 200, 83, 33, 0), (200, 15, 200, 185, 181, 0)])
    check(doc, "整数 7-8 人の表", [counts(f"integer-{s}", "7-8") for s in ("1e7", "1e9")],
          [(20, 10, 20, 10, 0, 0), (10, 2, 10, 8, 7, 0)])
    check(doc, "整数 9-10 人の表", [counts(f"integer-{s}", "9-10") for s in ("1e7", "1e9")],
          [(20, 3, 20, 17, 0, 0), (10, 0, 0, 10, 0, 10)])
    check(doc, "整数 11-14 人の表", [counts(f"integer-{s}", "11-14") for s in ("1e6", "1e7", "1e8", "1e9")],
          [(40, 22, 40, 18, 0, 0), (40, 13, 38, 27, 0, 2), (40, 11, 15, 29, 0, 27), (40, 4, 4, 36, 0, 36)])
    consistent = [r for (family, _), r in rows.items() if family.startswith("bnf") or family == "convex"]
    check(doc, "BNF・凸は 1 回だけ解いても既定でも全て正しく、解き直さない",
          all(r["fixed_correct"] == r["games"] == r["auto_correct"]
              and (r["refined"], r["rational"], r["approximate"]) == ("0", "0", "0") for r in consistent), True)
    integers = {key: r for key, r in rows.items() if key[0].startswith(("integer", "lower"))}
    check(doc, "9 人以上の整数ゲームで、正しくないのに保証が exact のままの配分の数",
          sum(int(r["incorrect_exact"]) for (_, players), r in integers.items() if players in ("9-10", "11-14")), 0)
    r = rows[("integer-1e8", "11-14")]
    check(doc, "scale = 1e8 の 11-14 人の (正しくない数, approximate の数)",
          (int(r["auto_solved"]) - int(r["auto_correct"]), int(r["approximate"])), (25, 27))
    check(doc, "8 人以下の整数ゲームで、正しくないのに保証が exact のままの配分がある行と数",
          {key: int(r["incorrect_exact"]) for key, r in integers.items()
           if key[1] in ("3-6", "7-8") and r["incorrect_exact"] != "0"}, {("integer-1e12", "3-6"): 80})
    large = [r for (_, players), r in rows.items() if players == "16-20"]
    overhead = [100 * (float(r["auto_seconds"]) / float(r["fixed_seconds"]) - 1) for r in large]
    check(doc, "16-20 人の確認による時間の増加 (%) の範囲", (round(min(overhead)), round(max(overhead))), (6, 14))
    r = rows[("integer-1e9", "7-8")]
    check(doc, "scale = 1e9 の 7-8 人の合計の秒 (1 回だけ解く, 既定)",
          (float(r["fixed_seconds"]), round(float(r["auto_seconds"]))), (0.02, 44))
    r = rows[("superadditive-float", "11-14")]
    check(doc, "浮動小数点数の和の優加法的なゲーム (ゲーム数, 1 回だけ解く: 合格, 既定: 合格, 解き直した数)",
          tuple(int(r[k]) for k in ("games", "fixed_correct", "auto_correct", "refined")), (40, 28, 28, 0))


def verify_refinement_rows():
    doc = "docs/performance.md (上限と既定値の根拠)"
    rows = read("data/analysis/refinement-rows.csv")
    check(doc, "解き直しの行数を調べた人数", sorted({int(r["players"]) for r in rows}), [11, 14, 16])
    first = {(r["family"], r["players"], r["seed"]): int(r["rows_added"]) for r in rows if r["ratio"] == "1e-7"}
    growth = [int(r["rows_added"]) / first[(r["family"], r["players"], r["seed"])]
              for r in rows if r["ratio"] != "1e-7" and r["certified"] == "true"]
    check(doc, "比を下げて合格した解の、追加した行の比 1e-7 に対する倍率の最大", round(max(growth), 1), 5.3)
    hard = [r for r in rows if (r["family"], r["players"]) == ("integer-1e10", "14")]
    failed = [r for r in hard if r["ratio"] == "1e-9"]
    check(doc, "値 1e10・14 人の比 1e-9 (状態, 秒の範囲)",
          ({r["status"] for r in failed}, round(min(float(r["seconds"]) for r in failed)),
           round(max(float(r["seconds"]) for r in failed))), ({"error"}, 9, 12))
    finest = [r for r in hard if r["ratio"] == "1e-11"]
    proper = 2 ** 14 - 2
    check(doc, "値 1e10・14 人の比 1e-11 (真部分提携に対する行の割合 %, 秒, 合格)",
          [(round(100 * int(r["rows_added"]) / proper), round(float(r["seconds"])), r["certified"]) for r in finest],
          [(92, 18, "false"), (92, 19, "false")])
    check(doc, "値 1e10 の 16 人は計測に含めていない",
          any((r["family"], r["players"]) == ("integer-1e10", "16") for r in rows), False)
    check(doc, "値 1e10・14 人の比 1e-11 の行数 (100 行単位)",
          [round(int(r["rows_added"]), -2) for r in finest], [15100, 15100])
    with open("src/nucleolus/mod.rs") as f:
        source = f.read()
    check("src/nucleolus/mod.rs (REFINEMENT_ROW_FACTOR の説明)", "同じ倍率を書いている",
          f"最初の {round(max(growth), 1)} 倍以下" in source, True)


def verify_exact_integer():
    doc = "docs/performance.md (上限と既定値の根拠)"
    rows = read("data/bench/exact-scaling-integer.csv")

    def seconds(n):
        values = [float(r["seconds"]) for r in rows if r["n"] == str(n)]
        return (min(values), max(values), len(values))

    check(doc, "整数ゲームの有理数の逐次 LP の秒 (最小, 最大, ゲーム数) n = 7, 8",
          [seconds(7), seconds(8)], [(0.9, 2.2, 4), (2.4, 9.8, 4)])
    check(doc, "同 n = 9, 10 (秒は四捨五入)",
          [tuple(round(v) for v in seconds(n)) for n in (9, 10)], [(47, 133, 4), (61, 288, 2)])

    def span(n):
        low, high, _ = seconds(n)
        return f"{round(low)}-{round(high)} 秒"

    with open("src/nucleolus/mod.rs") as f:
        source = f.read()
    with open("docs/guarantees.md") as f:
        guarantees = f.read()
    check("src/nucleolus/mod.rs (MAX_AUTO_RATIONAL_PLAYERS の説明)", "8・9 人の時間の範囲が同じ",
          f"8 人で {span(8)}、9 人で {span(9)}" in source, True)
    check("docs/guarantees.md (検証の範囲)", "8・9 人の時間の範囲が同じ",
          f"8 人は {span(8)}、9 人は {span(9)}" in guarantees, True)


def verify_auto_constants():
    """既定の手順の定数と、文書に書いた手順が同じか。"""
    doc = "docs/performance.md (仁の許容誤差の決め方)"
    with open("src/nucleolus/mod.rs") as f:
        source = f.read()
    with open("docs/performance.md") as f:
        text = f.read()

    def constant(name):
        return re.search(rf"const {name}: [^=]+ = ([^;]+);", source).group(1)

    check(doc, "順に試す許容誤差の比", constant("AUTO_TOLERANCES"), "[1e-7, 1e-9, 1e-11]")
    for claim in ["1. `max|v(S)|` の `1e-7` 倍の許容誤差で解く。",
                  "3. 満たさなければ、許容誤差を `1e-9` 倍、`1e-11` 倍に下げて解き直し、同じ確認をする。"]:
        check(doc, f"手順の記述: {claim[:24]}", claim in text, True)
    reading = (constant("MAX_AUTO_CHECK_PLAYERS"), constant("AUTO_READING"),
               constant("MAX_AUTO_RATIONAL_PLAYERS"), constant("AUTO_READING_WITH_RATIONAL"))
    check(doc, "確認する人数の上限と読み取りの比 (定数)", reading, ("20", "1e-11", "8", "1e-12"))
    claim = "2. {} 人以下なら、解いた配分が Kohlberg 基準を満たすかを、`max|v(S)|` の `{}` 倍 ({} 人以下では `{}` 倍) の許容誤差で確かめる。"
    check(doc, "確認の記述が定数と同じ", claim.format(*reading) in text, True)
    factor = constant("REFINEMENT_ROW_FACTOR")
    check(doc, "解き直しの行の上限 (定数と式)",
          (factor, "(REFINEMENT_ROW_FACTOR * result.rows_added).max(4 * n * n)" in source), ("8", True))
    check(doc, "行の上限の記述が定数と同じ",
          f"最初に解いたときの {factor} 倍 (少なくとも `4 n^2` 行) まで" in text, True)
    check(doc, "有理数に切り替える人数の記述が定数と同じ",
          f"4. どの許容誤差でも満たさなければ、{constant('MAX_AUTO_RATIONAL_PLAYERS')} 人以下は有理数の逐次 LP で求める。" in text, True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--instances", help="make_instances.sh で作ったディレクトリ")
    args = parser.parse_args()
    verify_comparison()
    verify_phase2_bench()
    verify_oracle_bench()
    verify_oracle_tolerance()
    verify_uncertainty_bench()
    verify_voting_bench()
    verify_certified()
    verify_sampled()
    verify_guarantees()
    verify_exact_solver()
    verify_variant_limits()
    verify_tolerance_study()
    verify_refinement_rows()
    verify_exact_integer()
    verify_auto_constants()
    if args.instances:
        verify_allocations(args.instances)
    print(f"\n{'食い違い ' + str(len(failures)) + ' 件' if failures else '全ての主張がデータと一致した'}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
