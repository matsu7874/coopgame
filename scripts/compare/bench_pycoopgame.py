"""pyCoopGame の仁を 1 回計算し、bench.R と同じ形の 1 行の CSV (file,n,seconds,x1 x2 ...) を出す。

使い方: PYCOOPGAME_DIR=... python bench_pycoopgame.py <nucleolus> <v.txt>
入力はビット順。pyCoopGame の nucleolus() は、空提携から始めて大きさ順・同じ大きさ内は辞書式の
DataFrame (列 coalition, value) を受け取り、GLPK で解く。プレ仁の関数はない。
"""
import contextlib, io, itertools, os, sys, time

sys.path.insert(0, os.environ["PYCOOPGAME_DIR"])
import pandas as pd  # noqa: E402
from pyCoopGame import Nucleolus  # noqa: E402

method, path = sys.argv[1], sys.argv[2]
if method != "nucleolus":
    sys.exit("Error: pyCoopGame has no prenucleolus")
v = [float(x) for x in open(path).read().split()]
n = (len(v) + 1).bit_length() - 1
coalitions = [c for k in range(n + 1) for c in itertools.combinations(range(n), k)]
game = pd.DataFrame({
    "coalition": [list(c) for c in coalitions],
    "value": [v[sum(1 << i for i in c) - 1] if c else 0.0 for c in coalitions],
})
start = time.perf_counter()
# nucleolus() は成否を標準出力に print するので、CSV と混ざらないよう捨てる。
with contextlib.redirect_stdout(io.StringIO()) as log:
    x = Nucleolus.nucleolus(game)
seconds = time.perf_counter() - start
if "properly calculated" not in log.getvalue():
    sys.exit("Error: " + log.getvalue().strip())
print(os.path.basename(path), n, seconds, " ".join(repr(x[i]) for i in range(n)), sep=",")
