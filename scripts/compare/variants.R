# 使い方: Rscript variants.R <v.txt>...  (ビット順の特性関数)
# CoopGame の perCapitaNucleolus・proportionalNucleolus・modiclus を「ファイル名 C|P|M 値...」の形で出す。
# 計算できない場合 (CoopGame が NULL を返す・エラー) は値の代わりに NA を出す。
suppressMessages(library(CoopGame))
show <- function(name, kind, f) {
  x <- tryCatch(suppressWarnings(f()), error = function(e) NULL)
  out <- capture.output(cat(name, kind, if (is.null(x)) "NA" else format(x, digits = 15), "\n"))
  writeLines(out)
}
for (f in commandArgs(TRUE)) {
  v <- scan(f, quiet = TRUE); n <- as.integer(round(log2(length(v) + 1)))
  lex <- unlist(lapply(1:n, function(k) apply(combn(n, k), 2, function(m) v[sum(2^(m - 1))])))
  invisible(capture.output(pc <- tryCatch(perCapitaNucleolus(lex), error = function(e) NULL)))
  invisible(capture.output(pr <- tryCatch(proportionalNucleolus(lex), error = function(e) NULL)))
  invisible(capture.output(mo <- tryCatch(modiclus(lex), error = function(e) NULL)))
  show(basename(f), "C", function() pc)
  show(basename(f), "P", function() pr)
  show(basename(f), "M", function() mo)
}
