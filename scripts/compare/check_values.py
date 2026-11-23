"""Shapley 値・Banzhaf 値を CoopGame と照合する。

使い方 (リポジトリのルートで、R_LIBS を setup.sh の出力どおりに設定して実行):
  python3 scripts/compare/check_values.py [--csv 出力.csv]

BNF タイプ 1, 2, 4、n = 4, 6, 8, 10、seed 9 のゲームで、coopgame-rs の厳密計算と
CoopGame 0.2.2 の shapleyValue・banzhafValue を比べ、最大の相対差を出す。
--csv を付けると、ゲームと値ごとの相対差を CSV に書く (data/compare/values.csv。
scripts/verify_claims.py が docs/reproducibility.md の数値と照合する)。
"""
import csv, os, subprocess, sys, tempfile

here = os.path.dirname(os.path.abspath(__file__))
rust = os.environ.get("COOPGAME", "target/release/coopgame")
csv_path = sys.argv[sys.argv.index("--csv") + 1] if "--csv" in sys.argv else None
records = []
with tempfile.TemporaryDirectory() as work:
    files = []
    for kind in (1, 2, 4):
        for n in (4, 6, 8, 10):
            path = os.path.join(work, f"t{kind}_n{n}.txt")
            with open(path, "w") as f:
                subprocess.run([rust, "generate", "--type", str(kind), "--n", str(n), "--seed", "9"], stdout=f, check=True)
            files.append(path)
    out = subprocess.run(["Rscript", os.path.join(here, "values.R"), *files], capture_output=True, text=True, check=True).stdout
    worst, cases = 0.0, 0
    for line in out.splitlines():
        name, kind, *values = line.split()
        reference = [float(v) for v in values]
        command = "shapley" if kind == "S" else "banzhaf"
        ours = [float(v) for v in subprocess.run([rust, command, os.path.join(work, name)], capture_output=True, text=True, check=True).stdout.split()]
        scale = max(1.0, max(abs(v) for v in reference))
        diff = max(abs(a - b) for a, b in zip(ours, reference)) / scale
        worst = max(worst, diff)
        cases += 1
        records.append((name[:-len(".txt")], command, f"{diff:.3e}"))
if csv_path:
    with open(csv_path, "w", newline="") as f:
        writer = csv.writer(f, lineterminator="\n")
        writer.writerow(["game", "value", "relative_difference"])
        writer.writerows(records)
print(f"{cases} 件を比べ、最大の相対差は {worst:.3e}")
sys.exit(0 if cases == 24 and worst < 1e-9 else 1)
