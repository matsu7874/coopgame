"""ドキュメントに書いた数値を、保存した CSV から再計算して照合する。

使い方 (リポジトリのルートで実行):
  python3 scripts/verify_claims.py
  python3 scripts/verify_claims.py --instances scripts/compare/work   # 配分の検証も行う

--instances には `scripts/compare/make_instances.sh` で作ったディレクトリ (small/, large/) を渡す。
渡すと、coopgame-rs の全配分の Kohlberg 判定と、誤りと判定した他実装の配分の再判定も行う
(target/release/coopgame が必要。`cargo build --release --features cli` で作る)。

照合する主張ごとに、どのドキュメントの記述かを示す。1 つでも食い違えば終了コード 1 を返す。

主要な表と数値は、ドキュメントの本文から読み取って CSV と比べる (`doc_table`・`doc_match` と `agrees`)。
ドキュメントの数値を書き換えれば、このスクリプトを変えなくても照合の対象が変わる。
数値の書き方の約束 (`agrees` が確かめる):
- 「X 以下」「X 以内」と上界として書く数は、データを表示の桁で切り上げた値 (X >= データ、X - データ < 表示の最小桁)。
- それ以外の数は、データを表示の桁で四捨五入した値。
- 「N 万」は 1 万を単位にした四捨五入、「1,234」のカンマは桁区切り。
"""
import argparse
import collections
import csv
import math
import os
import re
import subprocess
import sys
import tempfile

failures = []


check_count = [0]


def check(doc, claim, actual, expected):
    check_count[0] += 1
    ok = actual == expected
    mark = "ok  " if ok else "FAIL"
    print(f"[{mark}] {doc}: {claim}: {actual!r}" + ("" if ok else f" (期待値 {expected!r})"))
    if not ok:
        failures.append((doc, claim))


def read(path):
    with open(path) as f:
        return list(csv.DictReader(f))


# ---------------------------------------------------------------- ドキュメントの本文の読み取り
_texts = {}


def doc_text(path):
    if path not in _texts:
        with open(path, encoding="utf-8") as f:
            _texts[path] = f.read()
    return _texts[path]


def fail(doc, claim, detail):
    print(f"[FAIL] {doc}: {claim}: {detail}")
    failures.append((doc, claim))


def split_row(line):
    return [c.strip() for c in re.split(r"(?<!\\)\|", line.strip())[1:-1]]


def doc_table(path, header, keys, columns, key_columns=1):
    """見出しの行が header で始まる Markdown の表の、本文の行 (セルのリスト) を返す。

    columns は期待する列の数 (見出しと全ての行がこの列数であること)。
    keys は、行の先頭 key_columns 列 (1 列なら文字列、2 列以上ならタプル) の期待する並び。
    行が消えた・足された・入れ替わった、表の途中に空行がある、行の列数が見出しと異なる、
    のどれかなら失敗を記録して空のリストを返す (呼び出し側は照合を行わず、失敗だけが残る)。
    """
    lines = doc_text(path).splitlines()
    starts = [i for i, line in enumerate(lines) if line.startswith(header)]
    if len(starts) != 1:
        fail(path, f"表 {header!r}", f"見出しの行が {len(starts)} 個ある (1 個であるべき)")
        return []
    i = starts[0]
    width = len(split_row(lines[i]))
    rows = []
    end = i + 2
    for end in range(i + 2, len(lines) + 1):
        if end == len(lines) or not lines[end].startswith("|"):
            break
        rows.append(split_row(lines[end]))
    following = next((line for line in lines[end:] if line.strip()), "")
    ok = True
    if following.startswith("|"):
        fail(path, f"表 {header!r}", "表の途中に空行か表でない行がある")
        ok = False
    widths = sorted({len(r) for r in rows} | {width})
    if widths != [columns]:
        fail(path, f"表 {header!r}", f"見出しと行の列数 {widths} が期待する {columns} 列と異なる")
        ok = False
    actual = [r[0] if key_columns == 1 else tuple(r[:key_columns]) for r in rows]
    if actual != list(keys):
        fail(path, f"表 {header!r}", f"行のキー {actual} が期待 {list(keys)} と異なる")
        ok = False
    check_count[0] += 1
    return rows if ok else []


def doc_match(path, pattern):
    """本文で pattern (正規表現) に当たる最初の箇所のグループを返す。見つからなければ None。"""
    m = re.search(pattern, doc_text(path))
    if m is None:
        failures.append((path, pattern))
        print(f"[FAIL] {path}: {pattern!r} に当たる記述が見つからない")
    return m


NUMBER = re.compile(r"\d[\d,]*(?:\.\d+)?(?:e-?\d+)?")


def numbers(text):
    return NUMBER.findall(text)


def agrees(text, value, bound=False, unit=1.0):
    """ドキュメントの数 text が、データの値 value を表示の桁で丸めた (bound なら切り上げた) ものか。"""
    s = text.replace(",", "")
    shown = float(s) * unit
    mantissa, _, exponent = s.partition("e")
    places = len(mantissa.partition(".")[2])
    step = 10.0 ** ((int(exponent) if exponent else 0) - places) * unit
    slack = step * 1e-9
    if bound:
        return value <= shown + slack and shown - value < step + slack
    return abs(value - shown) <= step / 2 + slack


def check_number(doc, claim, text, value, bound=False, unit=1.0):
    check_count[0] += 1
    ok = agrees(text, value, bound, unit)
    mark = "ok  " if ok else "FAIL"
    kind = "切り上げ" if bound else "四捨五入"
    print(f"[{mark}] {doc}: {claim}: 本文 {text} / データ {value:.6g} ({kind})")
    if not ok:
        failures.append((doc, claim))


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

    check(doc, "破産ゲームは行を追加しない",
          {r["rows_added"] for r in rows if r["kind"].startswith("bankruptcy")}, {"0"})
    check(doc, "seed 数 (請求 1-100 の 100、請求 1-1000 の 100、投票 25)",
          tuple(sum((r["kind"], r["n"]) == key for r in rows)
                for key in (("bankruptcy", "100"), ("bankruptcy-wide", "100"), ("voting", "25"))),
          (3, 10, 2))
    wide = sorted(float(r["seconds"]) for r in rows if (r["kind"], r["n"]) == ("bankruptcy-wide", "100"))
    check(doc, "請求 1-1000 の 100 人で 23 秒以下の seed の数", sum(t <= 23 for t in wide), 9)


def verify_voting_bench():
    """拒否権のある重み付き投票ゲームの、修正前後の時間と最大メモリ、列挙の上限の根拠 (本文から読む)。"""
    doc = "docs/performance.md"
    before = {(r["kind"], int(r["n"])): r for r in read("data/bench/voting-oracle-before.csv")}
    after = {(r["kind"], int(r["n"])): r for r in read("data/bench/voting-oracle-after.csv")}
    check(doc, "修正後は全ての計測が完了した", {r["status"] for r in after.values()}, {"ok"})

    kinds = {"拒否権 3 人 (重み 1000 × 3、1 × (n-3))": "veto3", "拒否権 2 人 (重み 1000 × 2、1 × (n-2))": "veto2",
             "多数決 (重み 1)": "majority", "重み 1-100、基準は過半数": "random100", "重み 1-100、基準は 9/10": "random100q90"}
    labels = list(kinds)
    keys = [(labels[0], "20"), ("同上", "24"), ("同上", "26"), ("同上", "40"), ("同上", "60"),
            (labels[1], "24"), ("同上", "50"), (labels[2], "31"), (labels[3], "30"), (labels[4], "15")]
    statuses = {"6 GB を超えて停止": "rss_limit", "計測せず": "skipped"}
    kind = None
    for cells in doc_table(doc, "| ゲーム | n | 修正前 (秒 / MB)", keys, 4, key_columns=2):
        kind = kind if cells[0] == "同上" else kinds[cells[0]]
        n = int(cells[1])
        for label, rows, cell in (("修正前", before, cells[2]), ("修正後", after, cells[3])):
            r = rows.get((kind, n))
            claim = f"拒否権の表 {kind} の {n} 人 ({label})"
            if r is None:
                fail(doc, claim, "データにない")
            elif cell in statuses:
                check(doc, f"{claim} の状態", r["status"], statuses[cell])
            else:
                seconds, memory = numbers(cell)
                check(doc, f"{claim} の状態", r["status"], "ok")
                check_number(doc, f"{claim} の秒", seconds, float(r["seconds"]))
                check_number(doc, f"{claim} の MB", memory, float(r["max_rss_mb"]))

    veto = [r for (k, _), r in after.items() if k.startswith("veto")]
    m = doc_match(doc, r"拒否権のあるゲームの仁は、どの人数でも[^。]*との差が ([\d.e-]+) 以下")
    if m:
        check_number(doc, "拒否権のあるゲームの仁との差の上界", m.group(1), max(float(r["max_error"]) for r in veto), bound=True)
    others = [key for key in before if not key[0].startswith("veto")]
    m = doc_match(doc, r"拒否権のあるゲーム以外の (\d+) 組 \([^)]*、(\d+)-(\d+) 人\)")
    if m:
        check(doc, "拒否権のあるゲーム以外の組の数と人数の範囲", tuple(int(g) for g in m.groups()),
              (len(others), min(n for _, n in others), max(n for _, n in others)))
    check(doc, "拒否権のあるゲーム以外で、LP を解いた回数と追加した行が同じ",
          all((before[k]["lp_solves"], before[k]["rows_added"]) == (after[k]["lp_solves"], after[k]["rows_added"])
              for k in others), True)
    m = doc_match(doc, r"時間の差は修正前の (\d+)% と ([\d.]+) 秒の大きい方以内、最大メモリの増加は (\d+) MB 以下")
    if m:
        share, floor = float(m.group(1)) / 100, float(m.group(2))
        check(doc, "拒否権のあるゲーム以外で、時間の差が本文の基準以内",
              all(abs(float(after[k]["seconds"]) - float(before[k]["seconds"])) <= max(share * float(before[k]["seconds"]), floor)
                  for k in others), True)
        check_number(doc, "拒否権のあるゲーム以外の最大メモリの増加の上界", m.group(3),
                     max(int(after[k]["max_rss_mb"]) - int(before[k]["max_rss_mb"]) for k in others), bound=True)

    budget = read("data/bench/voting-budget.csv")
    with open("src/games/voting.rs", encoding="utf-8") as f:
        constant = int(re.search(r"const ENUMERATION_BUDGET: usize = (\d+);", f.read()).group(1))
    m = doc_match(doc, r"超過の順に (\d+) 個 \(`ENUMERATION_BUDGET`\) まで読み")
    if m:
        check(doc, "列挙の上限 ENUMERATION_BUDGET", int(m.group(1)), constant)

    def row(b, kind, n):
        return next(r for r in budget if (int(r["budget"]), r["kind"], int(r["n"])) == (b, kind, n))

    one_stage = ("majority", "apex", "random10", "random100", "random100q23")
    if doc_match(doc, r"仁が 1 段で決まるゲームでは、確定済みの提携のある探索で読んだ提携は全て 0 個だった"):
        check(doc, "仁が 1 段で決まるゲームでは、確定済みの提携のある探索で読んだ提携が全て 0 個",
              {r["max_read"] for r in budget if r["kind"] in one_stage}, {"0"})
    m = doc_match(doc, r"基準 9/10 の 10 人のゲームは最大 (\d+) 個を読み、上限 (\d+) では素数の探索に進む")
    if m:
        check(doc, "重み 1-100・基準 9/10 の 10 人 (最大の読んだ数, 小さい上限で上限まで読む)",
              (m.group(1), int(row(int(m.group(2)), "random100q90", 10)["searches_over_budget"]) > 0),
              (row(constant, "random100q90", 10)["max_read"], True))
    m = doc_match(doc, r"上限 (\d+) で ([\d.]+) 秒・(\d+) MB、(\d+) で ([\d.]+) 秒・(\d+) MB だった")
    if m:
        for b, seconds, memory in ((m.group(1), m.group(2), m.group(3)), (m.group(4), m.group(5), m.group(6))):
            r = row(int(b), "veto3", 40)
            check_number(doc, f"拒否権 3 人の 40 人、上限 {b} の秒", seconds, float(r["seconds"]))
            check_number(doc, f"拒否権 3 人の 40 人、上限 {b} の MB", memory, float(r["max_rss_mb"]))
    source = "src/games/voting.rs"
    m = doc_match(source, r"上限 256 で ([\d.]+) 秒・(\d+) MB、1024 で ([\d.]+) 秒・(\d+) MB、(?:\s|/)*4096 で ([\d.]+) 秒・(\d+) MB")
    if m:
        for b, seconds, memory in ((256, m.group(1), m.group(2)), (1024, m.group(3), m.group(4)), (4096, m.group(5), m.group(6))):
            r = row(b, "veto3", 40)
            check_number(source, f"定数の説明の上限 {b} の秒", seconds, float(r["seconds"]))
            check_number(source, f"定数の説明の上限 {b} の MB", memory, float(r["max_rss_mb"]))
    doc = "docs/large-games.md"
    m = doc_match(doc, r"拒否権のある 60 人のゲームを ([\d.]+) 秒で解く \(修正前は 24 人で (\d+) 秒・([\d.]+) GB")
    if m:
        check_number(doc, "拒否権 3 人の 60 人の秒", m.group(1), float(after[("veto3", 60)]["seconds"]))
        check_number(doc, "修正前の拒否権 3 人の 24 人の秒", m.group(2), float(before[("veto3", 24)]["seconds"]))
        check_number(doc, "修正前の拒否権 3 人の 24 人の GB (1000 MB 単位)", m.group(3),
                     float(before[("veto3", 24)]["max_rss_mb"]) / 1000)


def verify_oracle_tolerance():
    doc = "docs/performance.md"
    rows = read("data/bench/oracle-tolerance.csv")
    m = doc_match(doc, r"比 `([\de.-]+)`・`([\de.-]+)`・`([\de.-]+)`\n\(`oracle::nucleolus_with")
    if m:
        check(doc, "40 人の投票ゲームを解いた比", [r["ratio"] for r in rows], list(m.groups()))
    check(doc, "40 人の投票ゲームを全ての比で解けた", all(not r["max_error"].startswith("error") for r in rows), True)
    m = doc_match(doc, r"どれも ([\d.]+) 秒で解け、\n追加した行は (\d+)、仁 \([^)]*\) との差は ([\d.e-]+) だった")
    if m:
        check_number(doc, "40 人の投票ゲームの最大の秒", m.group(1), max(float(r["seconds"]) for r in rows))
        check(doc, "40 人の投票ゲームの追加した行", {int(r["rows_added"]) for r in rows}, {int(m.group(2))})
        check_number(doc, "40 人の投票ゲームの仁との差の最大", m.group(3), max(float(r["max_error"]) for r in rows))


def verify_uncertainty_bench():
    doc = "docs/performance.md (値の不確かさの分析の時間)"
    rows = read("data/bench/uncertainty.csv")
    # (分析, ゲーム, 区間の幅) -> 人数 -> mode -> 行
    pairs = collections.defaultdict(lambda: collections.defaultdict(dict))
    for r in rows:
        pairs[(r["analysis"], r["family"], r["relative"])][int(r["players"])][r["mode"]] = r

    def seconds(key, mode, digits=None):
        values = [float(m[mode]["seconds"]) for _, m in sorted(pairs[key].items())]
        return [round(v, digits) if digits is not None else float(f"{v:.2g}") for v in values]

    check(doc, "人数", {key: sorted(by_players) for key, by_players in pairs.items()},
          {("uncertainty", "bnf1", "1e-1"): [5, 8, 10, 12], ("influence", "bnf1", ""): [5, 8, 10, 12],
           ("uncertainty", "integer-1e9", "1e-1"): [4, 6, 8], ("influence", "integer-1e9", ""): [4, 6],
           ("uncertainty", "integer-1e9", "1e-12"): [4, 6]})
    key = ("uncertainty", "bnf1", "1e-1")
    check(doc, "uncertainty (±10%) BNF の秒 (1 回だけ解く, 既定)", (seconds(key, "fixed", 2), seconds(key, "auto", 2)),
          ([0.04, 0.41, 0.95, 2.19], [0.04, 0.41, 0.95, 2.23]))
    key = ("uncertainty", "integer-1e9", "1e-1")
    check(doc, "uncertainty (±10%) 整数の秒 (1 回だけ解く, 既定)", (seconds(key, "fixed", 2), seconds(key, "auto", 2)),
          ([0.02, 0.14, 0.44], [0.02, 0.14, 0.46]))
    key = ("influence", "bnf1", "")
    check(doc, "influence BNF の秒 (1 回だけ解く, 既定)", (seconds(key, "fixed"), seconds(key, "auto")),
          ([0.002, 0.033, 0.12, 0.34], [0.002, 0.033, 0.12, 0.35]))
    key = ("influence", "integer-1e9", "")
    check(doc, "influence 整数の秒 (1 回だけ解く, 既定) と (解き直した, 有理数, 解いた回数)",
          (seconds(key, "fixed"), [round(v, 2) if v < 1 else round(v, 1) for v in seconds(key, "auto", 3)],
           [(int(m["auto"]["refined"]), int(m["auto"]["rational"]), int(m["auto"]["solves"])) for _, m in sorted(pairs[key].items())]),
          ([0.003, 0.095], [0.09, 10.4], [(19, 14, 31), (90, 68, 127)]))
    key = ("uncertainty", "integer-1e9", "1e-12")
    check(doc, "uncertainty (±1e-12 倍) 整数: 1 回だけ解くとエラー、既定の秒と (解き直した, 有理数)",
          ([m["fixed"]["outcome"] for _, m in sorted(pairs[key].items())],
           [round(v, 2) if v < 10 else round(v, 1) for v in seconds(key, "auto", 3)],
           [(int(m["auto"]["refined"]), int(m["auto"]["rational"])) for _, m in sorted(pairs[key].items())]),
          (["buried", "buried"], [1.30, 37.8], [(200, 200), (200, 200)]))
    check(doc, "既定の手順は全ての行で分析を終える",
          all(m["auto"]["outcome"] == "ok" for by_players in pairs.values() for m in by_players.values()), True)
    plain = [m for by_players in pairs.values() for m in by_players.values()
             if m["auto"]["refined"] == "0" and m["fixed"]["outcome"] == "ok"]
    check(doc, "解き直しがない行は、既定でも解き直し・有理数・approximate が 0",
          all((m["auto"]["rational"], m["auto"]["approximate"]) == ("0", "0") for m in plain), True)
    ratios = [float(m["auto"]["seconds"]) / float(m["fixed"]["seconds"]) for m in plain]
    check(doc, "解き直しがない行の時間の比 (既定 / 1 回だけ解く) の範囲", (round(min(ratios), 2), round(max(ratios), 2)), (1.0, 1.1))
    slow = [m for by_players in pairs.values() for m in by_players.values()
            if m["auto"]["refined"] != "0" and m["fixed"]["outcome"] == "ok"]
    ratios = [float(m["auto"]["seconds"]) / float(m["fixed"]["seconds"]) for m in slow]
    check(doc, "解き直した influence の時間の比の範囲", (round(min(ratios)), round(max(ratios))), (30, 109))


def verify_guarantees():
    doc = "docs/guarantees.md"
    rows = read("data/analysis/assumption-study.csv")
    m = doc_match(doc, r"各クラス・各 n で seed 0-9 \(計 (\d+) ゲーム\)")
    if m:
        check(doc, "ゲーム数", len(rows), int(m.group(1)))
    check(doc, "未決は 0 個", sum(r["outcome"] == "undecided" for r in rows), 0)
    convex = [r for r in rows if r["convex"] == "true"]
    m = doc_match(doc, r"凸ゲーム (\d+) 個 \(convex (\d+)、superadditive のうち凸の (\d+)\) は全て合格した")
    if m:
        check(doc, "凸ゲームの数 (全体, convex, superadditive) と全て合格",
              (tuple(int(g) for g in m.groups()), {r["outcome"] for r in convex}),
              ((len(convex), sum(r["class"] == "convex" for r in convex), sum(r["class"] == "superadditive" for r in convex)),
               {"certified"}))
    nonconvex = [r for r in rows if r["convex"] == "false"]
    errors = [r for r in nonconvex if r["outcome"] == "error"]
    m = doc_match(doc, r"凸でないゲーム (\d+) 個では、(\d+) 個 \((\d+)%\) で手法が止まらないか")
    if m:
        check(doc, "凸でないゲームの数と失敗の数", (int(m.group(1)), int(m.group(2))), (len(nonconvex), len(errors)))
        check_number(doc, "凸でないゲームのうち失敗の割合 (%)", m.group(3), 100 * len(errors) / len(nonconvex))
    m = doc_match(doc, r"\(numerical\) が (\d+) 個、周回・反復の上限 \(limit\) が (\d+) 個")
    if m:
        check(doc, "失敗の内訳 (numerical, limit)", (int(m.group(1)), int(m.group(2))),
              tuple(sum(r["detail"] == d for r in errors) for d in ("numerical", "limit")))
    certified = [r for r in nonconvex if r["outcome"] == "certified"]
    m = doc_match(doc, r"凸でないのに仁を返したのは (\d+) 個で、大半は小さい n \(n = 4 が (\d+) 個\)")
    if m:
        check(doc, "凸でないのに合格した数と n = 4 の数", (int(m.group(1)), int(m.group(2))),
              (len(certified), sum(r["n"] == "4" for r in certified)))
    refuted = [r for r in rows if r["outcome"] == "refuted"]
    tally = collections.Counter((r["detail"], r["in_prekernel"]) for r in refuted)
    m = doc_match(doc, r"誤った配分を返した (\d+) 個は[\s\S]*?- (\d+) 個はプレ仁[\s\S]*?- (\d+) 個はプレカーネルの点だが[\s\S]*?- (\d+) 個はプレカーネル条件も")
    if m:
        check(doc, "否定の数と内訳 (配分集合の外, プレカーネルだが仁でない, プレカーネルでもない)",
              tuple(int(g) for g in m.groups()),
              (len(refuted), tally[("not_imputation", "true")], tally[("not_nucleolus", "true")], tally[("not_nucleolus", "false")]))

    def worst(rows, n, column):
        return max(float(r[column]) for r in rows if r["n"] == str(n))

    threshold = read("data/bench/auto-threshold.csv")
    m = doc_match(doc, r"`ConvexChecked` \| (\d+) 人以下は逐次 LP")
    if m:
        limit = int(m.group(1))
        above = min(int(r["n"]) for r in threshold if int(r["n"]) > limit)
        check(doc, f"境目: {limit} 人は LP が速く、{above} 人は凸ゲームの手法が速い",
              (worst(threshold, limit, "lp_seconds") < worst(threshold, limit, "convex_seconds"),
               worst(threshold, above, "convex_seconds") < worst(threshold, above, "lp_seconds")), (True, True))


def verify_exact_solver():
    doc = "docs/performance.md"
    rows = read("data/bench/exact-scaling.csv")

    def worst(n):
        return max(float(r["seconds"]) for r in rows if r["n"] == str(n))

    m = doc_match(doc, r"最大で n = 8 は ([\d.]+) 秒、n = 10 は ([\d.]+) 秒だった。\n(\d+) ゲーム全てで")
    if m:
        check_number(doc, "有理数の仁の n = 8 の最大の秒", m.group(1), worst(8))
        check_number(doc, "有理数の仁の n = 10 の最大の秒", m.group(2), worst(10))
        check(doc, "計測したゲーム数と全て一致", (len(rows), all(r["matches_certified"] == "true" for r in rows)),
              (int(m.group(3)), True))


def verify_sampled():
    doc = "docs/performance.md"
    rows = read("data/analysis/sampled-convergence.csv")
    check(doc, "収束の行数 (3 タイプ x 2 サイズ x 5 seed x 7 段階)", len(rows), 210)
    group = [r for r in rows if (r["type"], r["n"], r["pairs"]) == ("4", "10", "1600")]
    shares = [100 * int(r["evaluations"]) / (2 ** 10 - 2) for r in group]
    m = doc_match(doc, r"BNF タイプ 4 の n = 10 では、真部分提携の (\d+)-(\d+)% を評価しても相対誤差が最大 ([\d.]+)% 残った")
    if m:
        check_number(doc, "タイプ 4・n = 10・pairs = 1600 で評価した真部分提携の割合の最小 (%)", m.group(1), min(shares))
        check_number(doc, "同、割合の最大 (%)", m.group(2), max(shares))
        check_number(doc, "同、相対誤差の最大 (%)", m.group(3), 100 * max(float(r["relative_error"]) for r in group))


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

    limits = [("per-capita", "MAX_PER_CAPITA_PLAYERS"), ("proportional", "MAX_PROPORTIONAL_PLAYERS"),
              ("disruption", "MAX_DISRUPTION_PLAYERS"), ("bargaining", "MAX_BARGAINING_SET_PLAYERS")]
    for method, name in limits:
        largest = max(int(r["n"]) for r in rows if r["method"] == method and within(method, int(r["n"])))
        check(doc, f"{method}: 条件を満たす最大の人数が {name}", largest, constants[name])
        check(doc, f"{method}: 1 人多い人数を測り、条件を超えた", (bool(group(method, largest + 1)), within(method, largest + 1)), (True, False))
    check(doc, "modiclus の上限 7 人で条件を満たす", (constants["MAX_MODICLUS_PLAYERS"], within("modiclus", 7)), (7, True))
    check(doc, "基準 2 の投票ゲームの 11 人の交渉集合の LP の数", [int(r["lp_solves"]) for r in group("bargaining", 11) if r["game"] == "quota2"], [56210])


TOLERANCE_KEYS = [
    ("整数 (scale = 1・1e5)", "3-6"), ("整数 (scale = 1e6)", "3-6"), ("整数 (scale = 1e7)", "3-6"),
    ("整数 (scale = 1e8)", "3-6"), ("整数 (scale = 1e9)", "3-6"), ("整数 (scale = 1e12)", "3-6"),
    ("整数 (下限、scale = 1e7)", "3-6"), ("整数 (下限、scale = 1e9)", "3-6"), ("整数 (下限、scale = 1e10)", "3-6"),
    ("整数 (scale = 1e7)", "7-8"), ("整数 (scale = 1e9)", "7-8"), ("整数 (scale = 1e7)", "9-10"), ("整数 (scale = 1e9)", "9-10"),
    ("整数 (scale = 1e6)", "11-14"), ("整数 (scale = 1e7)", "11-14"), ("整数 (scale = 1e8)", "11-14"), ("整数 (scale = 1e9)", "11-14"),
    ("BNF タイプ 1・2・4", "3-6、11-14、16-20"), ("凸", "11-14、16-20"),
]


def tolerance_families(label):
    """許容誤差の表のゲームの列を、tolerance-study.csv の family の並びにする。"""
    if label == "BNF タイプ 1・2・4":
        return ["bnf1", "bnf2", "bnf4"]
    if label == "凸":
        return ["convex"]
    m = re.fullmatch(r"整数 \((下限、)?scale = ([\de・]+)\)", label)
    prefix = "lower" if m.group(1) else "integer"
    return [f"{prefix}-{'1e0' if s == '1' else s}" for s in m.group(2).split("・")]


def verify_tolerance_study():
    doc = "docs/performance.md"
    rows = {(r["family"], r["players"]): r for r in read("data/analysis/tolerance-study.csv")}
    columns = ("games", "fixed_correct", "auto_correct", "refined", "rational", "approximate")
    for cells in doc_table(doc, "| ゲーム | 人数 | ゲーム数 | 1 回だけ解く", TOLERANCE_KEYS, 8, key_columns=2):
        players = cells[1].split("、")
        games = re.sub(r"^各 ", "", cells[2]).split("、")
        if len(games) != len(players):
            fail(doc, f"許容誤差の表 {cells[0]}", "人数とゲーム数の数が異なる")
            continue
        for family in tolerance_families(cells[0]):
            for group, count in zip(players, games):
                r = rows.get((family, group))
                claim = f"許容誤差の表 {family} の {group} 人"
                if r is None:
                    fail(doc, claim, "データにない")
                    continue
                expected = [count] + [r["games"] if c == "全て" else c for c in cells[3:]]
                check(doc, f"{claim} ({', '.join(columns)})", [r[c] for c in columns], expected)

    large = [r for (_, players), r in rows.items() if players == "16-20"]
    m = doc_match(doc, r"16-20 人の (\d+) ゲームの合計で (\d+)-(\d+)%")
    if m:
        overhead = [100 * (float(r["auto_seconds"]) / float(r["fixed_seconds"]) - 1) for r in large]
        check(doc, "16-20 人のゲーム数", {r["games"] for r in large}, {m.group(1)})
        check_number(doc, "16-20 人の確認による時間の増加の最小 (%)", m.group(2), min(overhead))
        check_number(doc, "16-20 人の確認による時間の増加の最大 (%)", m.group(3), max(overhead))
    integers = {key: r for key, r in rows.items() if key[0].startswith(("integer", "lower"))}
    m = doc_match(doc, r"9 人以上の整数ゲームで正しくなかった配分は、全て保証が `approximate` になった \(保証が `exact` のまま正しくなかったものは (\d+) 個\)")
    if m:
        check(doc, "9 人以上の整数ゲームで、正しくないのに保証が exact のままの配分の数",
              sum(int(r["incorrect_exact"]) for (_, players), r in integers.items() if players in ("9-10", "11-14")),
              int(m.group(1)))
    m = doc_match(doc, r"scale = 1e8 の 11-14 人では、正しくない (\d+) 個に対して `approximate` は (\d+) 個")
    if m:
        r = rows[("integer-1e8", "11-14")]
        check(doc, "scale = 1e8 の 11-14 人の (正しくない数, approximate の数)", (int(m.group(1)), int(m.group(2))),
              (int(r["auto_solved"]) - int(r["auto_correct"]), int(r["approximate"])))
    m = doc_match(doc, r"値が `1e12` 程度の整数ゲームでは[\s\S]*?(\d+) 個のうち (\d+) 個のずれを見分けられなかった")
    if m:
        r = rows[("integer-1e12", "3-6")]
        check(doc, "scale = 1e12 の (ゲーム数, 正しくないのに exact)", (int(m.group(1)), int(m.group(2))),
              (int(r["games"]), int(r["incorrect_exact"])))
        check(doc, "8 人以下の整数ゲームで、正しくないのに exact のままの配分は scale = 1e12 の行だけ",
              [key for key, r in integers.items() if key[1] in ("3-6", "7-8") and r["incorrect_exact"] != "0"],
              [("integer-1e12", "3-6")])
    m = doc_match(doc, r"scale = 1e9 の 7-8 人では、(\d+) ゲームの合計が ([\d.]+) 秒から ([\d.]+) 秒になった")
    if m:
        r = rows[("integer-1e9", "7-8")]
        check(doc, "scale = 1e9 の 7-8 人のゲーム数", r["games"], m.group(1))
        check_number(doc, "scale = 1e9 の 7-8 人の合計の秒 (1 回だけ解く)", m.group(2), float(r["fixed_seconds"]))
        check_number(doc, "scale = 1e9 の 7-8 人の合計の秒 (既定)", m.group(3), float(r["auto_seconds"]))
    m = doc_match(doc, r"浮動小数点数の和で作った優加法的なゲーム \((\d+-\d+) 人、(\d+) 個\) は、どちらの解き方でも (\d+) 個が厳密な検証に合格し、(\d+) 個が合格しなかった。\n\s*既定の手順は (\d+) 個とも解き直していない")
    if m:
        r = rows[("superadditive-float", m.group(1))]
        check(doc, "浮動小数点数の和の優加法的なゲーム (数, 合格, 不合格, 解き直さなかった数)",
              tuple(int(g) for g in m.groups()[1:]),
              (int(r["games"]), int(r["fixed_correct"]) if r["fixed_correct"] == r["auto_correct"] else -1,
               int(r["games"]) - int(r["auto_correct"]), int(r["games"]) - int(r["refined"])))


def verify_refinement_rows():
    doc = "docs/performance.md"
    rows = read("data/analysis/refinement-rows.csv")
    m = doc_match(doc, r"(\d+)・(\d+)・(\d+) 人の整数ゲームを、比 `1e-7`・`1e-9`・`1e-11` の許容誤差で 1 回ずつ解いた")
    if m:
        check(doc, "解き直しの行数を調べた人数", sorted({int(r["players"]) for r in rows}), [int(g) for g in m.groups()])
    check(doc, "解き直しの行数を調べた比", sorted({r["ratio"] for r in rows}), ["1e-11", "1e-7", "1e-9"])
    first = {(r["family"], r["players"], r["seed"]): int(r["rows_added"]) for r in rows if r["ratio"] == "1e-7"}
    growth = [int(r["rows_added"]) / first[(r["family"], r["players"], r["seed"])]
              for r in rows if r["ratio"] != "1e-7" and r["certified"] == "true"]
    m = doc_match(doc, r"追加した行は比 `1e-7` のときの ([\d.]+) 倍以下だった")
    if m:
        check_number(doc, "比を下げて合格した解の、追加した行の比 1e-7 に対する倍率の上界", m.group(1), max(growth), bound=True)
    source = "src/nucleolus/mod.rs"
    m = doc_match(source, r"追加した行は最初の ([\d.]+) 倍以下だった")
    if m:
        check_number(source, "REFINEMENT_ROW_FACTOR の説明の倍率の上界", m.group(1), max(growth), bound=True)
    hard = [r for r in rows if (r["family"], r["players"]) == ("integer-1e10", "14")]
    failed = [r for r in hard if r["ratio"] == "1e-9"]
    finest = [r for r in hard if r["ratio"] == "1e-11"]
    m = doc_match(doc, r"値が `1e10` 程度の 14 人の (\d+) ゲームは、比 `1e-9` では (\d+)-(\d+) 秒かけて失敗し、比 `1e-11` では真部分提携の (\d+)% \(約 ([\d,]+) 行\) を追加して\n\s*(\d+)-(\d+) 秒かかり、どちらも合格しなかった")
    if m:
        proper = 2 ** 14 - 2
        check(doc, "値 1e10・14 人のゲーム数", (len(failed), len(finest)), (int(m.group(1)), int(m.group(1))))
        check(doc, "値 1e10・14 人の比 1e-9 は全て失敗", {r["status"] for r in failed}, {"error"})
        check_number(doc, "値 1e10・14 人の比 1e-9 の秒の最小", m.group(2), min(float(r["seconds"]) for r in failed))
        check_number(doc, "値 1e10・14 人の比 1e-9 の秒の最大", m.group(3), max(float(r["seconds"]) for r in failed))
        for r in finest:
            check_number(doc, "値 1e10・14 人の比 1e-11 の真部分提携に対する行の割合 (%)", m.group(4),
                         100 * int(r["rows_added"]) / proper)
            check(doc, "値 1e10・14 人の比 1e-11 の行数 (100 行単位)", round(int(r["rows_added"]), -2), int(m.group(5).replace(",", "")))
        check_number(doc, "値 1e10・14 人の比 1e-11 の秒の最小", m.group(6), min(float(r["seconds"]) for r in finest))
        check_number(doc, "値 1e10・14 人の比 1e-11 の秒の最大", m.group(7), max(float(r["seconds"]) for r in finest))
        check(doc, "値 1e10・14 人の比 1e-11 は合格しなかった", {r["certified"] for r in finest}, {"false"})
    if doc_match(doc, r"値が `1e10` 程度の 16 人のゲームは[^。]*計測に含めていない"):
        check(doc, "値 1e10 の 16 人は計測に含めていない",
              any((r["family"], r["players"]) == ("integer-1e10", "16") for r in rows), False)


def verify_exact_integer():
    doc = "docs/performance.md"
    rows = read("data/bench/exact-scaling-integer.csv")

    def seconds(n):
        return [float(r["seconds"]) for r in rows if r["n"] == str(n)]

    for cells in doc_table(doc, "| 人数 | 整数ゲーム (scale = 1・1e9)", ["7", "8", "9", "10"], 2):
        n = int(cells[0])
        low, high = numbers(cells[1])[:2]
        check_number(doc, f"整数ゲームの有理数の逐次 LP {n} 人の秒の最小", low, min(seconds(n)))
        check_number(doc, f"整数ゲームの有理数の逐次 LP {n} 人の秒の最大", high, max(seconds(n)))
        per_scale = collections.Counter(r["type"] for r in rows if r["n"] == str(n))
        expected = 1 if "各 scale で 1 ゲーム" in cells[1] else 2
        check(doc, f"整数ゲームの有理数の逐次 LP {n} 人の scale ごとのゲーム数", dict(per_scale),
              {"integer-1e0": expected, "integer-1e9": expected})
    for path, pattern in (("src/nucleolus/mod.rs", r"8 人で (\d+)-(\d+) 秒、9 人で (\d+)-(\d+) 秒"),
                          ("docs/guarantees.md", r"8 人は (\d+)-(\d+) 秒、9 人は (\d+)-(\d+) 秒")):
        m = doc_match(path, pattern)
        if m:
            for n, low, high in ((8, m.group(1), m.group(2)), (9, m.group(3), m.group(4))):
                check_number(path, f"{n} 人の有理数の逐次 LP の秒の最小", low, min(seconds(n)))
                check_number(path, f"{n} 人の有理数の逐次 LP の秒の最大", high, max(seconds(n)))


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


# ---------------------------------------------------------------- ドキュメントの本文から読む表
def verify_comparison_text():
    doc = "docs/comparison.md"
    small = read("data/compare/small.csv")
    large = read("data/compare/large.csv")
    ok_rows = [r for r in small + large if r["status"] == "ok"]

    def worst(impl, method, n):
        return max(float(r["seconds"]) for r in ok_rows if (r["impl"], r["method"], r["n"]) == (impl, method, str(n)))

    impls = ("coopgame-rs", "coopgame-rs-full", "CoopGame", "TUGLab")
    rows = doc_table(doc, "| n | coopgame-rs 制約生成", ["8", "10", "12", "14", "16", "18"], 5)
    for cells in rows:
        for impl, cell in zip(impls, cells[1:]):
            check_number(doc, f"実行時間の表 n = {cells[0]} の {impl} (仁の最大の秒)", numbers(cell)[0],
                         worst(impl, "nucleolus", cells[0]))
    m = doc_match(doc, r"n = 18 では制約生成 ([\d.]+) 秒、CoopGame ([\d.]+) 秒")
    if m:
        check_number(doc, "n = 18 のプレ仁 (制約生成)", m.group(1), worst("coopgame-rs", "prenucleolus", 18))
        check_number(doc, "n = 18 のプレ仁 (CoopGame)", m.group(2), worst("CoopGame", "prenucleolus", 18))

    certified = read("data/compare/certified.csv")

    def distances(impls, passed):
        return [float(r["distance_to_certified"]) for r in certified
                if r["impl"] in impls and (r["certified"] == "true") == passed]

    table = {r[0]: r for r in doc_table(doc, "| 実装 | 合格 | 不合格 | 厳密な仁", ["coopgame-rs (2 方式)", "CoopGame", "TUGLab"], 4)}
    ours = ("coopgame-rs", "coopgame-rs-full")
    if table:
        verify_certified_table(doc, table, certified, distances, ours)


def verify_certified_table(doc, table, certified, distances, ours):
    row = table.get("coopgame-rs (2 方式)", ["", "", "", ""])
    check(doc, "厳密な検証の表 coopgame-rs の (合格, 不合格)", (row[1], row[2]),
          (str(len(distances(ours, True))), str(len(distances(ours, False)))))
    check_number(doc, "厳密な検証の表 coopgame-rs の距離の上界", numbers(row[3])[0], max(distances(ours, True)), bound=True)
    for impl in ("CoopGame", "TUGLab"):
        row = table.get(impl, ["", "", "", ""])
        check(doc, f"厳密な検証の表 {impl} の (合格, 不合格)", (row[1], row[2]),
              (str(len(distances((impl,), True))), str(len(distances((impl,), False)))))
        passed = re.search(r"合格は ([\d.e-]+) 以下", row[3])
        check_number(doc, f"厳密な検証の表 {impl} の合格の距離の上界", passed.group(1) if passed else "0",
                     max(distances((impl,), True)), bound=True)
    m = re.search(r"n = 17, 18 で ([\d.]+) と ([\d.]+)", table.get("CoopGame", [""] * 4)[3])
    for n, text in zip(("17", "18"), m.groups() if m else ("0", "0")):
        values = {float(r["distance_to_certified"]) for r in certified
                  if (r["impl"], r["n"], r["certified"]) == ("CoopGame", n, "false")}
        check(doc, f"CoopGame の不合格 n = {n} の距離は仁・プレ仁で同じ", len(values), 1)
        check_number(doc, f"CoopGame の不合格 n = {n} の距離", text, max(values))
    m = re.search(r"不合格は ([\d.]+)-([\d.]+)", table.get("TUGLab", [""] * 4)[3])
    failed = distances(("TUGLab",), False)
    check_number(doc, "TUGLab の不合格の距離の最小", m.group(1) if m else "0", min(failed))
    check_number(doc, "TUGLab の不合格の距離の最大", m.group(2) if m else "0", max(failed))
    m = doc_match(doc, r"x\* から ([\d.]+) 以上離れて")
    if m:
        check(doc, "不合格の配分は x* から本文の値以上離れている",
              min(distances(("CoopGame", "TUGLab"), False)) >= float(m.group(1)), True)



def verify_variants_text():
    doc = "docs/comparison.md"
    variants = read("data/compare/variants.csv")
    m = doc_match(doc, r"seed 0-3 \((\d+) 個\) と、CoopGame のヘルプの例など (\d+) 個")
    games = sorted({r["game"] for r in variants})
    if m:
        check(doc, "仁の変種の比較のゲーム数 (BNF, その他)", (int(m.group(1)), int(m.group(2))),
              (sum(g.startswith("t") for g in games), sum(not g.startswith("t") for g in games)))
    tally = collections.Counter((r["solution"], r["status"]) for r in variants)
    m = doc_match(doc, r"per capita 仁は (\d+) ゲーム全てで一致した。比例仁は (\d+) ゲームで一致し、残る (\d+) 個は両方とも計算しなかった")
    if m:
        check(doc, "仁の変種の比較 (per capita の一致, 比例仁の一致, 両方とも計算しない)",
              tuple(int(g) for g in m.groups()),
              (tally[("per-capita", "match")], tally[("proportional", "match")], tally[("proportional", "both_na")]))
    check(doc, "仁の変種の比較に不一致がない", sum(r["status"] == "mismatch" for r in variants), 0)
    m = doc_match(doc, r"差は相対 ([\d.e-]+) 以下")
    if m:
        check_number(doc, "仁の変種の比較の相対差の上界", m.group(1),
                     max(float(r["relative_difference"]) for r in variants if r["status"] == "match"), bound=True)


def verify_values_text():
    doc = "docs/reproducibility.md"
    rows = read("data/compare/values.csv")
    m = doc_match(doc, r"`data/compare/values.csv`[^|]*\| (\d+) ゲーム (\d+) 件が相対 ([\d.e-]+) 以内で一致")
    if m:
        check(doc, "Shapley 値・Banzhaf 値の比較 (ゲーム数, 件数)", (int(m.group(1)), int(m.group(2))),
              (len({r["game"] for r in rows}), len(rows)))
        check_number(doc, "Shapley 値・Banzhaf 値の相対差の上界", m.group(3),
                     max(float(r["relative_difference"]) for r in rows), bound=True)


def verify_variants_record():
    doc = "docs/reproducibility.md"
    rows = read("data/compare/variants.csv")
    m = doc_match(doc, r"`data/compare/variants.csv`[^|]*\| per capita 仁は (\d+) ゲームで一致。比例仁は (\d+) ゲームで一致し、(\d+) 件は両方とも計算しない。相対 ([\d.e-]+) 以内")
    if m:
        tally = collections.Counter((r["solution"], r["status"]) for r in rows)
        check(doc, "仁の変種の照合 (per capita の一致, 比例仁の一致, 両方とも計算しない)",
              tuple(int(g) for g in m.groups()[:3]),
              (tally[("per-capita", "match")], tally[("proportional", "match")], tally[("proportional", "both_na")]))
        check_number(doc, "仁の変種の照合の相対差の上界", m.group(4),
                     max(float(r["relative_difference"]) for r in rows if r["status"] == "match"), bound=True)


def verify_hadamard_primes():
    """Pashkovich の補題 4 の素数の数 (原典の log2(n!) 個と、Hadamard の上界による組) の記述。"""
    n = 30
    count = math.ceil(math.log2(math.factorial(n)))
    primes = []
    candidate = 2
    while len(primes) < count:
        if all(candidate % p for p in primes if p * p <= candidate):
            primes.append(candidate)
        candidate += 1
    doc = "src/games/voting.rs"
    m = doc_match(doc, r"30 人で、素数は (\d+) 個 \(和 ([\d,]+)\) から (\d+) 個 \(和 (\d+)\) に減る")
    if m:
        check(doc, "原典の素数の組 (小さい順に log2(30!) 個以上) の数と和", (int(m.group(1)), int(m.group(2).replace(",", ""))),
              (count, sum(primes)))
        # Hadamard の上界の組の数と和は src/games/voting.rs のテスト (hadamard_primes(30)) が確かめる
        check(doc, "Hadamard の上界の素数の組 (テストの値)", (m.group(3), m.group(4)),
              tuple(re.search(r"hadamard_primes\(30\)\.len\(\), (\d+)\);[\s\S]*?hadamard_primes\(30\)\.iter\(\)\.sum::<u64>\(\), (\d+)\);",
                              doc_text(doc)).groups()))
    doc = "docs/references.md"
    m = doc_match(doc, r"30 人で素数は (\d+) 個から (\d+) 個になる")
    if m:
        check(doc, "素数の数 (原典, Hadamard の上界)", int(m.group(1)), count)


def verify_prekernel_study():
    """プレカーネル・カーネルの形の分析 (references.md の凸ゲーム・0-単調なゲーム、performance.md のカーネル全体の時間)。"""
    rows = read("data/analysis/prekernel-study.csv")
    check("data/analysis/prekernel-study.csv", "全てのゲームでカーネル・プレカーネルを求められた",
          {(r["pre_status"], r["ker_status"]) for r in rows}, {("ok", "ok")})
    doc = "docs/references.md"
    convex = [r for r in rows if r["convex"] == "true"]
    m = doc_match(doc, r"分析の凸ゲーム (\d+) 個でカーネルが 1 点")
    if m:
        check(doc, "分析の凸ゲームの数と、カーネルが全て 1 点", (int(m.group(1)), all(r["ker_single"] == "true" for r in convex)),
              (len(convex), True))
    zero = [r for r in rows if r["zero_monotonic"] == "true"]
    m = doc_match(doc, r"分析の 0-単調なゲーム (\d+) 個で形が一致")
    if m:
        check(doc, "分析の 0-単調なゲームの数と、カーネルとプレカーネルの形 (多面体の数と最大の次元) が一致",
              (int(m.group(1)), all((r["pre_pieces"], r["pre_max_dim"]) == (r["ker_pieces"], r["ker_max_dim"]) for r in zero)),
              (len(zero), True))
    doc = "docs/performance.md"
    m = doc_match(doc, r"カーネル全体 \(`kernel-set`\) は、[^。]*n = 3-5 \(各 (\d+) ゲーム\) で最大 ([\d.]+) 秒、\s*n = 6 \(各 (\d+) ゲーム\) で ([\d.]+)-([\d.]+) 秒")
    if m:
        def count(n):
            return collections.Counter(r["class"] for r in rows if r["n"] == n)
        small = [float(r["ker_seconds"]) for r in rows if int(r["n"]) <= 5]
        six = [float(r["ker_seconds"]) for r in rows if r["n"] == "6"]
        check(doc, "カーネル全体の計測のゲーム数 (n = 3-5 の各クラス・各 n, n = 6 の各クラス)",
              (int(m.group(1)), int(m.group(3))), (min(count("3").values()), min(count("6").values())))
        check(doc, "カーネル全体の計測で、各クラス・各 n のゲーム数が同じ",
              all(len(set(count(n).values())) == 1 for n in ("3", "4", "5", "6")), True)
        check_number(doc, "カーネル全体の n = 3-5 の最大の秒", m.group(2), max(small))
        check_number(doc, "カーネル全体の n = 6 の最小の秒", m.group(4), min(six))
        check_number(doc, "カーネル全体の n = 6 の最大の秒", m.group(5), max(six))
    if doc_match(doc, r"調べたゲームのカーネルは 1 点か、線分をいくつか合わせた形だった"):
        check(doc, "調べたゲームのカーネルの最大の次元は 1 以下", max(int(r["ker_max_dim"]) for r in rows) <= 1, True)


def verify_performance_text():
    doc = "docs/performance.md"
    bench = []
    for name in ("results-n4-12.csv", "results-n13-16.csv", "results-n17-22.csv"):
        bench += read(os.path.join("data", "bench", name))

    def worst(method, n, column):
        return max(float(r[column]) for r in bench if (r["method"], r["n"]) == (method, str(n)))

    columns = [("nucleolus", "seconds"), ("nucleolus", "rows_added"), ("nucleolus-full", "seconds"),
               ("kernel", "seconds"), ("kernel", "iterations")]
    for cells in doc_table(doc, "| n | 仁 (制約生成) | 追加した行", ["8", "12", "16", "20", "22"], 6):
        for (method, column), cell in zip(columns, cells[1:]):
            if cell == "-":
                check(doc, f"明示ベクトルの表 n = {cells[0]} の {method} は計測していない",
                      any((r["method"], r["n"]) == (method, cells[0]) for r in bench), False)
            else:
                check_number(doc, f"明示ベクトルの表 n = {cells[0]} の {method} {column} (最大)", cell,
                             worst(method, cells[0], column))
    m = doc_match(doc, r"n = 16 で約 (\d+) 倍速い")
    if m:
        ratio = worst("nucleolus-full", 16, "seconds") / worst("nucleolus", 16, "seconds")
        check(doc, f"n = 16 の全行 / 制約生成の比 ({ratio:.1f}) を 2 桁に丸めた値", float(m.group(1)), float(f"{ratio:.2g}"))

    oracle = read("data/bench/oracle-scaling.csv")
    kinds = {"破産ゲーム (請求 1-100)": "bankruptcy", "破産ゲーム (請求 1-1000)": "bankruptcy-wide",
             "多数決ゲーム (重み 1)": "majority", "重み付き投票ゲーム (重み 1-10)": "voting"}
    oracle_keys = [("破産ゲーム (請求 1-100)", n) for n in ("40", "70", "100")] + \
        [("破産ゲーム (請求 1-1000)", n) for n in ("40", "70", "100")] + \
        [("多数決ゲーム (重み 1)", "21"), ("重み付き投票ゲーム (重み 1-10)", "25")]
    for cells in doc_table(doc, "| ゲーム | n | seed 数 | 時間 | 段の数", oracle_keys, 7, key_columns=2):
        kind, n = kinds.get(cells[0]), cells[1]
        group = [r for r in oracle if (r["kind"], r["n"]) == (kind, n)]
        label = f"オラクルの表 {kind} n = {n}"
        check(doc, f"{label} の seed 数", cells[2], str(len(group)))
        if not group:
            continue
        check_number(doc, f"{label} の時間 (最大)", cells[3], max(float(r["seconds"]) for r in group))
        check(doc, f"{label} の段の数 (最大)", cells[4], str(max(int(r["levels"]) for r in group)))
        check(doc, f"{label} の追加した行 (最大)", cells[5], str(max(int(r["rows_added"]) for r in group)))
        if kind.startswith("bankruptcy") or kind == "majority":
            check_number(doc, f"{label} の差 (最大)", numbers(cells[6])[-1],
                         max(float(r["max_error"]) for r in group), bound="以下" in cells[6])

    limits = read("data/bench/variant-limits.csv")
    names = {"per capita 仁": "per-capita", "比例仁": "proportional", "disruption nucleolus": "disruption",
             "modiclus": "modiclus", "交渉集合の判定": "bargaining"}
    for cells in doc_table(doc, "| 対象 | 上限 | 上限の人数での最大", list(names), 4):
        method = names.get(cells[0])
        limit = int(numbers(cells[1])[0])
        for n, cell in ((limit, cells[2]), (limit + 1, cells[3])):
            group = [r for r in limits if (r["method"], int(r["n"])) == (method, n)]
            if cell == "計測していない":
                check(doc, f"上限の表 {method} {n} 人は計測していない", group, [])
                continue
            seconds, memory = numbers(cell)
            label = f"上限の表 {method} {n} 人"
            check_number(doc, f"{label} の秒 (最大)", seconds, max(float(r["seconds"]) for r in group) if group else -1)
            check_number(doc, f"{label} の MB (最大)", memory, max(float(r["max_rss_mb"]) for r in group) if group else -1)
    m = doc_match(doc, r"18 人では、計測した (\d+) 例のうち (\d+) 例で最大メモリが 1 GB を超える")
    if m:
        group = [r for r in limits if r["method"] in ("per-capita", "proportional") and r["n"] == "18"]
        check(doc, "per capita 仁・比例仁の 18 人で 1 GB を超えた例 (計測数, 超えた数)",
              (int(m.group(1)), int(m.group(2))), (len(group), sum(int(r["max_rss_mb"]) > 1024 for r in group)))
    m = doc_match(doc, r"17 人では、計測した例で最大メモリが 1 GB を超える")
    if m:
        group = [r for r in limits if r["method"] == "disruption" and r["n"] == "17"]
        check(doc, "disruption nucleolus の 17 人の全ての例で 1 GB を超えた",
              (len(group) > 0, all(int(r["max_rss_mb"]) > 1024 for r in group)), (True, True))


def verify_guarantees_text():
    doc = "docs/guarantees.md"
    threshold = read("data/bench/auto-threshold.csv")
    columns = ("lp_seconds", "convex_seconds", "relative_difference")
    for cells in doc_table(doc, "| n | 逐次 LP (秒)", ["12", "16", "18", "20"], 4):
        group = [r for r in threshold if r["n"] == cells[0]]
        for column, cell in zip(columns, cells[1:]):
            check_number(doc, f"境目の表 n = {cells[0]} の {column} (最大)", cell,
                         max(float(r[column]) for r in group) if group else -1)

    convex = {"誘導部分グラフ": read("data/bench/convex-graph.csv"), "破産": read("data/bench/convex-bankruptcy.csv")}
    convex_keys = [("誘導部分グラフ", n) for n in ("16", "24", "32", "48", "64")] + [("破産", "20"), ("破産", "40")]
    for cells in doc_table(doc, "| ゲーム | n | 時間 (秒) | 周回", convex_keys, 6, key_columns=2):
        group = [r for r in convex.get(cells[0], []) if r["n"] == cells[1]]
        label = f"凸ゲームの手法の表 {cells[0]} n = {cells[1]}"
        if not group:
            check(doc, f"{label} のデータ", 0, 3)
            continue
        check_number(doc, f"{label} の時間 (最大)", cells[2], max(float(r["seconds"]) for r in group))
        check(doc, f"{label} の周回 (最大)", cells[3], str(max(int(r["sweeps"]) for r in group)))
        check_number(doc, f"{label} の評価回数 (最大、万)", numbers(cells[4])[0],
                     max(float(r["evaluations"]) for r in group), unit=1e4)
        found = numbers(cells[5])
        if "反例なし" in cells[5]:
            check(doc, f"{label} のカーネル条件の反例なし", {r["pair_check"] for r in group}, {"no_counterexample"})
        elif found:
            errors = [float(r["max_error"]) for r in group if r["max_error"]]
            check_number(doc, f"{label} の基準との差 (最大)", found[0], max(errors) if errors else -1)
    m = doc_match(doc, r"表の範囲での差は ([\d.]+)-([\d.]+)%")
    if m:
        # 表の評価回数 (各 n の最大) の行で、足す回数 2n + 2 + minimizations が占める割合
        share = []
        for cells in doc_table(doc, "| ゲーム | n | 時間 (秒) | 周回", convex_keys, 6, key_columns=2):
            group = [r for r in convex.get(cells[0], []) if r["n"] == cells[1]]
            if group:
                r = max(group, key=lambda r: int(r["evaluations"]))
                share.append(100 * (2 * int(r["n"]) + 2 + int(r["minimizations"])) / int(r["evaluations"]))
        if not share:
            return
        check_number(doc, "評価回数の差の割合の最小 (%)", m.group(1), min(share))
        check_number(doc, "評価回数の差の割合の最大 (%)", m.group(2), max(share))

    study = read("data/analysis/assumption-study.csv")
    for cells in doc_table(doc, "| クラス | ゲーム数 | 実際に凸", ["convex", "bnf1", "bnf2", "bnf4", "superadditive", "voting"], 6):
        group = [r for r in study if r["class"] == cells[0]]
        tally = collections.Counter(r["outcome"] for r in group)
        check(doc, f"仮定の実験の表 {cells[0]} (数, 凸, 合格, 否定, 失敗)", tuple(cells[1:6]),
              tuple(str(v) for v in (len(group), sum(r["convex"] == "true" for r in group),
                                     tally["certified"], tally["refuted"], tally["error"])))
    m = doc_match(doc, r"検証に合格した結果は、全て LP の仁との差が ([\d.e-]+) 以下")
    if m:
        check_number(doc, "合格した結果の LP の仁との差の上界", m.group(1),
                     max(float(r["distance_nucleolus"]) for r in study if r["outcome"] == "certified"), bound=True)


# 照合した項目の数の下限。照合の関数が表や記述を見失って何も照合しなくなったときに気づくため。
# 照合を足したり外したりしたら、出力の最後に表示する数に合わせて更新する。
MIN_CHECKS = 429


def run_all():
    for verify in (verify_comparison, verify_phase2_bench, verify_oracle_bench, verify_oracle_tolerance, verify_uncertainty_bench,
                   verify_voting_bench, verify_certified, verify_sampled, verify_guarantees, verify_exact_solver,
                   verify_variant_limits, verify_tolerance_study, verify_refinement_rows, verify_exact_integer,
                   verify_auto_constants, verify_comparison_text, verify_variants_text, verify_values_text,
                   verify_variants_record, verify_hadamard_primes, verify_prekernel_study, verify_performance_text,
                   verify_guarantees_text):
        verify()


def mutate_table(text, header, how):
    """header の表を壊す: 本文の 2 行目を消す (drop_row)、本文の 2 行目の前に空行を入れる (blank_line)、最後の列を消す (drop_column)。"""
    lines = text.split("\n")
    start = next(i for i, line in enumerate(lines) if line.startswith(header))
    end = next(i for i in range(start, len(lines) + 1) if i == len(lines) or not lines[i].startswith("|"))
    if how == "drop_row":
        del lines[start + 3]
    elif how == "blank_line":
        lines.insert(start + 3, "")
    elif how == "drop_column":
        for i in range(start, end):
            lines[i] = re.sub(r"(?<!\\)\|[^|]*\|$", "|", lines[i])
    return "\n".join(lines)


SELF_TEST_TABLES = [
    ("docs/comparison.md", "| n | coopgame-rs 制約生成"),
    ("docs/comparison.md", "| 実装 | 合格 | 不合格 | 厳密な仁"),
    ("docs/performance.md", "| ゲーム | 人数 | ゲーム数 | 1 回だけ解く"),
    ("docs/performance.md", "| 人数 | 整数ゲーム (scale = 1・1e9)"),
    ("docs/performance.md", "| ゲーム | n | 修正前 (秒 / MB)"),
    ("docs/performance.md", "| n | 仁 (制約生成) | 追加した行"),
    ("docs/performance.md", "| ゲーム | n | seed 数 | 時間 | 段の数"),
    ("docs/performance.md", "| 対象 | 上限 | 上限の人数での最大"),
    ("docs/guarantees.md", "| n | 逐次 LP (秒)"),
    ("docs/guarantees.md", "| ゲーム | n | 時間 (秒) | 周回"),
    ("docs/guarantees.md", "| クラス | ゲーム数 | 実際に凸"),
]


def self_test():
    """文書の表を壊したときに照合が失敗することを確かめる (文書のファイルは書き換えない)。"""
    import contextlib
    import io

    def attempt(path=None, text=None):
        failures.clear()
        check_count[0] = 0
        _texts.clear()
        if path:
            _texts[path] = text
        with contextlib.redirect_stdout(io.StringIO()):
            try:
                run_all()
            except Exception as error:  # 壊した表で照合が例外になるのも、検出できたとみなす
                return f"例外 {type(error).__name__}"
        if failures:
            return f"失敗 {len(failures)} 件"
        if check_count[0] < MIN_CHECKS:
            return f"照合数 {check_count[0]} が下限 {MIN_CHECKS} 未満"
        return None

    problems = []
    baseline = attempt()
    print(f"壊していない文書: {baseline or '全て一致'}")
    if baseline:
        problems.append("壊していない文書で照合が失敗した")
    for path, header in SELF_TEST_TABLES:
        with open(path, encoding="utf-8") as f:
            original = f.read()
        for how in ("drop_row", "blank_line", "drop_column"):
            detected = attempt(path, mutate_table(original, header, how))
            print(f"{path} {header!r} {how}: {detected or '検出できなかった'}")
            if not detected:
                problems.append((path, header, how))
    print(f"\n{'自己テストの不合格 ' + str(len(problems)) + ' 件' if problems else '自己テストに合格した'}")
    sys.exit(1 if problems else 0)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--instances", help="make_instances.sh で作ったディレクトリ")
    parser.add_argument("--self-test", action="store_true", help="文書の表を壊すと照合が失敗することを確かめる")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    run_all()
    if args.instances:
        verify_allocations(args.instances)
    if check_count[0] < MIN_CHECKS:
        fail("scripts/verify_claims.py", "照合した項目の数", f"{check_count[0]} が下限 {MIN_CHECKS} 未満")
    print(f"\n照合した項目 {check_count[0]} 件 (下限 {MIN_CHECKS})")
    print(f"{'食い違い ' + str(len(failures)) + ' 件' if failures else '全ての主張がデータと一致した'}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
