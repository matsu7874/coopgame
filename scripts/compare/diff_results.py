"""run.py の 2 つの出力を、時間以外 (状態と配分) で比べる。

使い方: python3 diff_results.py <保存した CSV> <再実行した CSV>
再実行した CSV に含まれる (実装, 手法, インスタンス) だけを比べる。配分は相対 1e-9 で比べる。
状態は種類 (ok, error, timeout, skipped) で比べる。エラーの文言は run.py の版で記録の仕方が違うため比べない
(results/small.csv はエラーを "Execution halted" と記録した版で作った)。
"""
import csv, sys

def load(path):
    with open(path) as f:
        return {(r["impl"], r["method"], r["type"], r["n"], r["seed"]): r for r in csv.DictReader(f)}

stored, rerun = load(sys.argv[1]), load(sys.argv[2])
mismatches = 0
for key, row in rerun.items():
    old = stored[key]
    if old["status"].split(":")[0] != row["status"].split(":")[0]:
        mismatches += 1
        print("状態が異なる", key, old["status"], row["status"])
        continue
    if row["status"] == "ok":
        a = [float(v) for v in old["allocation"].split()]
        b = [float(v) for v in row["allocation"].split()]
        scale = max(1.0, max(abs(v) for v in a))
        if len(a) != len(b) or max(abs(x - y) for x, y in zip(a, b)) > 1e-9 * scale:
            mismatches += 1
            print("配分が異なる", key)
print(f"{len(rerun)} 件を比べ、食い違いは {mismatches} 件")
sys.exit(1 if mismatches else 0)
