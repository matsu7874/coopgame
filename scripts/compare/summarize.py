"""results.csv を集計する: 実装ごとの最大時間・成否・配分の一致。

使い方: python3 summarize.py results.csv [インスタンスのディレクトリ]
インスタンスを渡すと、制約生成版と食い違った配分を Kohlberg 基準で判定する。
"""
import collections, csv, os, subprocess, sys, tempfile

rows = list(csv.DictReader(open(sys.argv[1])))
instances = sys.argv[2] if len(sys.argv) > 2 else None
rust = os.environ.get("COOPGAME", "target/release/coopgame")

def allocation(row):
    return [float(v) for v in row["allocation"].split()]

reference = {}
for r in rows:
    if r["impl"] == "coopgame-rs" and r["status"] == "ok":
        reference[(r["method"], r["type"], r["n"], r["seed"])] = allocation(r)

def kohlberg(row):
    """配分を coopgame verify で判定する。"""
    name = f"t{row['type']}_n{row['n']}_s{row['seed']}.txt"
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as sol:
        sol.write("\n".join(row["allocation"].split()) + "\n")
    cmd = [rust, "verify", os.path.join(instances, name), sol.name]
    if row["method"] == "prenucleolus":
        cmd.append("--pre")
    out = subprocess.run(cmd, capture_output=True, text=True).stdout
    os.unlink(sol.name)
    return "kohlberg=true" in out

status = collections.Counter()
agree = collections.Counter()
mismatches = []
times = collections.defaultdict(list)
for r in rows:
    key = (r["impl"], r["method"])
    status[key + (r["status"].split(":")[0],)] += 1
    if r["status"] != "ok":
        continue
    times[key + (int(r["n"]),)].append(float(r["seconds"]))
    ref = reference.get((r["method"], r["type"], r["n"], r["seed"]))
    if ref is None:
        continue
    x = allocation(r)
    scale = max(1.0, max(abs(v) for v in ref))
    same = len(x) == len(ref) and max(abs(a - b) for a, b in zip(x, ref)) <= 1e-6 * scale
    agree[key + (same,)] += 1
    if not same:
        mismatches.append(r)

print("## 成否")
for k in sorted(status):
    print(*k, status[k], sep="\t")
print("\n## 制約生成版との一致 (相対 1e-6)")
for k in sorted(agree, key=str):
    print(*k, agree[k], sep="\t")
print("\n## 最大時間 (秒)")
impls = sorted({k[0] for k in times})
for method in ["nucleolus", "prenucleolus"]:
    print(method, *impls, sep="\t")
    for n in sorted({k[2] for k in times}):
        cells = [f"{max(times[(i, method, n)]):.4f}" if (i, method, n) in times else "-" for i in impls]
        print(n, *cells, sep="\t")
if mismatches and instances:
    print("\n## 食い違った配分の Kohlberg 判定")
    verdict = collections.Counter()
    for r in mismatches:
        verdict[(r["impl"], r["method"], kohlberg(r))] += 1
    for k in sorted(verdict, key=str):
        print(*k, verdict[k], sep="\t")
