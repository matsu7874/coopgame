"""per capita 仁・比例仁・modiclus を CoopGame と照合する。

使い方 (リポジトリのルートで、R_LIBS を setup.sh の出力どおりに設定して実行):
  python3 scripts/compare/check_variants.py

BNF タイプ 1, 2, 4、n = 3, 4, 5、seed 0-3 のゲームと、CoopGame のヘルプにある例で、
coopgame-rs の `per-capita`・`proportional`・`modiclus` と CoopGame 0.2.2 の結果を比べる。
CoopGame が計算しないゲーム (比例仁で負の値を持つゲームなど) は、coopgame-rs もエラーを返すことを確かめる。
"""
import os, subprocess, sys, tempfile

here = os.path.dirname(os.path.abspath(__file__))
rust = os.environ.get("COOPGAME", "target/release/coopgame")
# CoopGame のヘルプの例 (辞書式順)
EXAMPLES = {
    "doc_modiclus": [0, 0, 0, 0, 5, 5, 8, 9, 10, 8, 13, 15, 16, 17, 21],
    "doc_proportional": [0, 0, 0, 48, 60, 72, 140],
    "doc_small": [1, 1, 1, 2, 3, 4, 5],
    # Young (1985) の費用分担の例 (CoopGame の costSharingGameVector(n=3, C=c(15,20,55,35,61,65,78)) の節約ゲーム)
    "young1985": [0, 0, 0, 0, 9, 10, 12],
    # 負の値を持つゲーム (比例仁は両方とも計算しない)
    "ferguson": [-1, 0, 1, 3, 4, 2, 5],
}


def lex_to_binary(lex):
    import itertools
    n = {3: 2, 7: 3, 15: 4, 31: 5}[len(lex)]
    order = [sum(1 << i for i in c) for k in range(1, n + 1) for c in itertools.combinations(range(n), k)]
    binary = [0.0] * len(lex)
    for mask, value in zip(order, lex):
        binary[mask - 1] = value
    return binary


with tempfile.TemporaryDirectory() as work:
    files = []
    for kind in (1, 2, 4):
        for n in (3, 4, 5):
            for seed in range(4):
                path = os.path.join(work, f"t{kind}_n{n}_s{seed}.txt")
                with open(path, "w") as f:
                    subprocess.run([rust, "generate", "--type", str(kind), "--n", str(n), "--seed", str(seed)], stdout=f, check=True)
                files.append(path)
    for name, lex in EXAMPLES.items():
        path = os.path.join(work, name + ".txt")
        with open(path, "w") as f:
            f.write("\n".join(str(v) for v in lex_to_binary(lex)) + "\n")
        files.append(path)
    out = subprocess.run(["Rscript", os.path.join(here, "variants.R"), *files], capture_output=True, text=True, check=True).stdout
    commands = {"C": "per-capita", "P": "proportional", "M": "modiclus"}
    tally = {kind: {"match": 0, "both_na": 0, "mismatch": []} for kind in commands}
    worst = 0.0
    for line in out.splitlines():
        name, kind, *values = line.split()
        result = subprocess.run([rust, commands[kind], os.path.join(work, name)], capture_output=True, text=True)
        ours = [float(v) for v in result.stdout.split()] if result.returncode == 0 else None
        if values == ["NA"]:
            if ours is None:
                tally[kind]["both_na"] += 1
            else:
                tally[kind]["mismatch"].append((name, "CoopGame は計算しない", ours))
            continue
        reference = [float(v) for v in values]
        if ours is None:
            tally[kind]["mismatch"].append((name, reference, result.stderr.strip()))
            continue
        scale = max(1.0, max(abs(v) for v in reference))
        diff = max(abs(a - b) for a, b in zip(ours, reference)) / scale
        if diff <= 1e-6:
            tally[kind]["match"] += 1
            worst = max(worst, diff)
        else:
            tally[kind]["mismatch"].append((name, reference, ours))
    for kind, t in tally.items():
        print(f"{commands[kind]}: 一致 {t['match']}、両方とも計算しない {t['both_na']}、不一致 {len(t['mismatch'])}")
        for m in t["mismatch"]:
            print("  ", *m)
    print(f"一致したものの最大の相対差 {worst:.1e}")
    sys.exit(1 if any(t["mismatch"] for t in tally.values()) else 0)
