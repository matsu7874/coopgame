"""同じインスタンスを coopgame-rs・CoopGame・TUGLab で解き、時間と配分を CSV で標準出力に出す。

使い方:
  R_LIBS=... TUGLAB_DIR=... python3 run.py <インスタンスのディレクトリ> [TUGLab の tol]

インスタンスは `coopgame generate` の出力 (ビット順) で、名前は t{タイプ}_n{人数}_s{seed}.txt。
1 回の計算の上限は TIMEOUT 秒で、時間切れになった (実装, 手法, タイプ) はそれより大きい n を省く。
"""
import csv, os, re, subprocess, sys

TIMEOUT = 300
here = os.path.dirname(os.path.abspath(__file__))
inst = sys.argv[1]
tol = sys.argv[2] if len(sys.argv) > 2 else None
rust = os.environ.get("COOPGAME", os.path.join(here, "..", "..", "target", "release", "coopgame"))
impls = os.environ.get("IMPLS", "coopgame-rs,coopgame-rs-full,CoopGame,TUGLab").split(",")


def key(name):
    t, n, s = re.match(r"t(\d)_n(\d+)_s(\d+)", name).groups()
    return int(t), int(n), int(s)


def run(impl, method, path):
    """(秒, 配分の文字列) を返す。"""
    if impl.startswith("coopgame-rs"):
        cmd = [rust, "nucleolus", path]
        cmd += ["--pre"] if method == "prenucleolus" else []
        cmd += ["--full"] if impl.endswith("full") else []
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT, check=True)
        seconds = float(re.search(r"time=([0-9.]+)s", r.stderr).group(1))
        return seconds, " ".join(r.stdout.split())
    cmd = ["Rscript", os.path.join(here, "bench.R"), impl, method, path] + ([tol] if tol else [])
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT)
    if r.returncode != 0:
        lines = [l for l in r.stderr.strip().splitlines() if l.startswith("Error")]
        raise RuntimeError((lines or ["unknown"])[-1])
    _, _, seconds, allocation = r.stdout.strip().splitlines()[-1].split(",", 3)
    return float(seconds), " ".join(allocation.split())


out = csv.writer(sys.stdout)
out.writerow(["impl", "method", "type", "n", "seed", "status", "seconds", "allocation"])
timed_out = set()
for name in sorted(os.listdir(inst), key=key):
    t, n, s = key(name)
    for method in ["nucleolus", "prenucleolus"]:
        for impl in impls:
            if (impl, method, t) in timed_out:
                out.writerow([impl, method, t, n, s, "skipped", "", ""])
                continue
            try:
                seconds, allocation = run(impl, method, os.path.join(inst, name))
                out.writerow([impl, method, t, n, s, "ok", f"{seconds:.6f}", allocation])
            except subprocess.TimeoutExpired:
                timed_out.add((impl, method, t))
                out.writerow([impl, method, t, n, s, "timeout", "", ""])
            except Exception as err:  # noqa: BLE001
                out.writerow([impl, method, t, n, s, "error:" + str(err).replace(",", ";")[:200], "", ""])
            sys.stdout.flush()
