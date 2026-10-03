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
    exact = [r for r in rows if r["kind"] in ("bankruptcy", "majority")]
    check(doc, "破産ゲーム・多数決ゲームの基準との差が全て 2e-12 以下",
          max(float(r["max_error"]) for r in exact) <= 2e-12, True)
    check(doc, "破産ゲームは行を追加しない", {r["rows_added"] for r in rows if r["kind"] == "bankruptcy"}, {"0"})

    def worst(kind, n, column):
        return max(float(r[column]) for r in rows if (r["kind"], r["n"]) == (kind, n))

    check(doc, "表の最大時間 (破産 40, 70, 100、多数決 21、投票 25)",
          (round(worst("bankruptcy", "40", "seconds"), 2), round(worst("bankruptcy", "70", "seconds"), 1),
           round(worst("bankruptcy", "100", "seconds"), 1), round(worst("majority", "21", "seconds"), 1),
           round(worst("voting", "25", "seconds"), 1)),
          (0.68, 25.6, 15.0, 4.7, 15.6))
    check(doc, "表の追加行数 (多数決 21、投票 25)",
          (int(worst("majority", "21", "rows_added")), int(worst("voting", "25", "rows_added"))), (3962, 6280))


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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--instances", help="make_instances.sh で作ったディレクトリ")
    args = parser.parse_args()
    verify_comparison()
    verify_phase2_bench()
    verify_oracle_bench()
    verify_certified()
    verify_sampled()
    verify_guarantees()
    verify_exact_solver()
    if args.instances:
        verify_allocations(args.instances)
    print(f"\n{'食い違い ' + str(len(failures)) + ' 件' if failures else '全ての主張がデータと一致した'}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
