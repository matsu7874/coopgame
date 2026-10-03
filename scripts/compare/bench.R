# 使い方: Rscript bench.R <CoopGame|TUGLab> <nucleolus|prenucleolus> <v.txt> [TUGLab の tol]
# 1 行の CSV (file,n,seconds,x1 x2 ...) を出す。入力はビット順。
args <- commandArgs(TRUE)
impl <- args[1]; method <- args[2]; file <- args[3]
# TUGLab の既定の許容誤差は 100 * .Machine$double.eps。
tol <- if (length(args) >= 4) as.numeric(args[4]) else 100 * .Machine$double.eps
v <- scan(file, quiet = TRUE)
n <- as.integer(round(log2(length(v) + 1)))
if (impl == "CoopGame") {
  suppressMessages(library(CoopGame))
  # CoopGame は辞書式順 (大きさ順、同じ大きさ内は辞書式) を要求する。
  lex <- unlist(lapply(1:n, function(k) {
    combos <- combn(n, k)
    apply(combos, 2, function(members) v[sum(2^(members - 1))])
  }))
  run <- if (method == "nucleolus") function() nucleolus(lex) else function() prenucleolus(lex)
} else {
  dir <- Sys.getenv("TUGLAB_DIR")
  for (f in list.files(dir, "\\.R$", full.names = TRUE)) {
    if (!grepl("zzz\\.R$", f)) source(f)
  }
  run <- if (method == "nucleolus") {
    function() nucleolusvalue(v, binary = TRUE, tol = tol)
  } else {
    function() prenucleolusvalue(v, binary = TRUE, tol = tol)
  }
}
elapsed <- system.time(x <- run())[["elapsed"]]
cat(basename(file), n, elapsed, paste(format(x, digits = 15), collapse = " "), sep = ",")
cat("\n")
