"""Shapley 値・Banzhaf 値を CoopGame と照合する。

使い方 (リポジトリのルートで、R_LIBS を setup.sh の出力どおりに設定して実行):
  python3 scripts/compare/check_values.py

BNF タイプ 1, 2, 4、n = 4, 6, 8, 10、seed 9 のゲームで、coopgame-rs の厳密計算と
CoopGame 0.2.2 の shapleyValue・banzhafValue を比べ、最大の相対差を出す。
"""
import os, subprocess, sys, tempfile

here = os.path.dirname(os.path.abspath(__file__))
rust = os.environ.get("COOPGAME", "target/release/coopgame")
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
        worst = max(worst, max(abs(a - b) for a, b in zip(ours, reference)) / scale)
        cases += 1
print(f"{cases} 件を比べ、最大の相対差は {worst:.3e}")
sys.exit(0 if cases == 24 and worst < 1e-9 else 1)
