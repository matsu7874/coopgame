#!/usr/bin/env bash
# 比較用のインスタンスを作る。
#   small/: BNF タイプ 1-5、n = 4-12、seed 0-2 (タイプ 5 は n >= 7)
#   large/: BNF タイプ 1, 2, 4、n = 13-18、seed 0
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin="${COOPGAME:-$here/../../target/release/coopgame}"
out="${1:-$here/work}"
mkdir -p "$out/small" "$out/large"
for t in 1 2 3 4 5; do
  for n in $(seq 4 12); do
    for s in 0 1 2; do
      "$bin" generate --type "$t" --n "$n" --seed "$s" > "$out/small/t${t}_n${n}_s${s}.txt" 2>/dev/null \
        || rm -f "$out/small/t${t}_n${n}_s${s}.txt"
    done
  done
done
for t in 1 2 4; do
  for n in $(seq 13 18); do
    "$bin" generate --type "$t" --n "$n" --seed 0 > "$out/large/t${t}_n${n}_s0.txt"
  done
done
