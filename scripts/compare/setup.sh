#!/usr/bin/env bash
# 比較対象の R 環境を用意する (Ubuntu 24.04 で確認)。
# CRAN に届かない環境を想定し、R パッケージは GitHub の CRAN ミラー (github.com/cran) から入れる。
# 比較に使った版を再現するため、各パッケージはコミットを固定して取得する。
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="${WORK:-$here/work}"
mkdir -p "$work/rlib"

# 比較時の版: r-base-core 4.3.3-2build2, r-cran-geometry 0.4.7-1, r-cran-gtools 3.9.5-1, libgmp-dev 6.3.0
sudo apt-get install -y r-base-core r-base-dev r-cran-geometry r-cran-gtools libgmp-dev

# パッケージ名 コミット (DESCRIPTION の Version)
pins=(
  "rcdd e85259b3a68fae7d1832cef4592f678e6545abf7"      # 1.6-1
  "CoopGame 250940376f550789b7a2b862e177e46b0aeb9c44"  # 0.2.2
  "TUGLab 3c3e2ba04d002b7838190c75d6bbbd45bb0590ac"    # 0.0.1
)
for pin in "${pins[@]}"; do
  read -r pkg commit <<< "$pin"
  if [ ! -d "$work/$pkg" ]; then
    git init -q "$work/$pkg"
    git -C "$work/$pkg" remote add origin "https://github.com/cran/$pkg.git"
  fi
  git -C "$work/$pkg" fetch -q --depth 1 origin "$commit"
  git -C "$work/$pkg" checkout -q --detach "$commit"
done
R CMD INSTALL -l "$work/rlib" "$work/rcdd"
R_LIBS="$work/rlib" R CMD INSTALL -l "$work/rlib" "$work/CoopGame"
# TUGLab は依存の volesti などのビルドが重いので、インストールせず R/ 以下を source して使う。
echo "export R_LIBS=$work/rlib TUGLAB_DIR=$work/TUGLab/R"
