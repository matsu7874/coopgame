# 使い方: Rscript values.R <v.txt>...  (ビット順の特性関数)
# CoopGame の shapleyValue と banzhafValue を「ファイル名 S|B 値...」の形で出す。
suppressMessages(library(CoopGame))
for (f in commandArgs(TRUE)) {
  v <- scan(f, quiet = TRUE); n <- as.integer(round(log2(length(v) + 1)))
  lex <- unlist(lapply(1:n, function(k) apply(combn(n, k), 2, function(m) v[sum(2^(m - 1))])))
  cat(basename(f), "S", format(shapleyValue(lex), digits = 15), "\n")
  cat(basename(f), "B", format(banzhafValue(lex), digits = 15), "\n")
}
