"""run.py の結果の配分を、全て有理数で厳密に検証する (coopgame certify)。

使い方 (リポジトリのルートで実行):
  python3 scripts/compare/certify_results.py scripts/compare/work > data/compare/certified.csv

small.csv と large.csv の、計算に成功した全ての配分 (全実装) について、
厳密な配分の復元と有理数の Kohlberg 判定を行い、合否と復元した配分との差を出す。

distance_to_certified は、同じインスタンスで coopgame-rs (制約生成) の配分から復元・合格した
厳密な仁 x* と、その行の配分との差の最大値。仁は一意なので、x* が合格していれば、
この差が丸め誤差を大きく超える配分は仁ではない。
"""
from fractions import Fraction
import csv, os, subprocess, sys, tempfile

instances = sys.argv[1]
rust = os.environ.get("COOPGAME", "target/release/coopgame")
here = os.path.dirname(os.path.abspath(__file__))
out = csv.writer(sys.stdout)
out.writerow(["impl", "method", "type", "n", "seed", "certified", "levels", "max_difference", "distance_to_certified", "reason"])
certified_nucleolus = {}  # (method, type, n, seed) -> 厳密な仁 (coopgame-rs の配分から復元・合格したもの)
for name, folder in (("small.csv", "small"), ("large.csv", "large")):
    with open(os.path.join(here, "..", "..", "data", "compare", name)) as f:
        for row in csv.DictReader(f):
            if row["status"] != "ok":
                continue
            game = os.path.join(instances, folder, f"t{row['type']}_n{row['n']}_s{row['seed']}.txt")
            with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as sol:
                sol.write("\n".join(row["allocation"].split()) + "\n")
            command = [rust, "certify", game, sol.name] + (["--pre"] if row["method"] == "prenucleolus" else [])
            # 1 行目が判定結果。不合格のときは 2 行目に CLI のエラーが続く。
            result = subprocess.run(command, capture_output=True, text=True)
            stderr = result.stderr.splitlines()[0]
            key = (row["method"], row["type"], row["n"], row["seed"])
            if row["impl"] == "coopgame-rs" and "certified=true" in stderr:
                certified_nucleolus[key] = [Fraction(v) for v in result.stdout.split()]
            distance = ""
            if key in certified_nucleolus:
                values = [Fraction(v) for v in row["allocation"].split()]
                distance = f"{float(max(abs(a - b) for a, b in zip(values, certified_nucleolus[key]))):e}"
            os.unlink(sol.name)
            fields = dict(item.split("=", 1) for item in stderr.split("reason=")[0].split() if "=" in item)
            reason = stderr.split("reason=", 1)[1].strip().replace(",", ";") if "reason=" in stderr else ""
            out.writerow([row["impl"], row["method"], row["type"], row["n"], row["seed"],
                          fields.get("certified", "error"), fields.get("levels", ""),
                          fields.get("max_difference", ""), distance, reason])
            sys.stdout.flush()
