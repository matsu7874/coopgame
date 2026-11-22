#!/usr/bin/env bash
# 比較対象の R 環境を用意する (Ubuntu 24.04 と macOS の Homebrew で確認)。
# 比較に使った版を再現するため、rcdd・CoopGame・TUGLab はコミットを固定して
# GitHub の CRAN ミラー (github.com/cran) から取得する。
# 作業ディレクトリは WORK で変えられる (既定はこのディレクトリの work/)。
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="${WORK:-$here/work}"
mkdir -p "$work/rlib"

case "$(uname -s)" in
  Linux)
    # 比較時の版: r-base-core 4.3.3-2build2, r-cran-geometry 0.4.7-1, r-cran-gtools 3.9.5-1, libgmp-dev 6.3.0
    sudo apt-get install -y r-base-core r-base-dev r-cran-geometry r-cran-gtools libgmp-dev
    ;;
  Darwin)
    # R と GMP は Homebrew で入れる。CoopGame が依存する geometry と gtools は、
    # CRAN から rlib に入れる (版は固定しない。入った版は末尾に表示する)。
    brew install r gmp
    # install.packages は失敗しても警告を出すだけなので、入ったかを確かめて止める。
    Rscript -e '
      lib <- commandArgs(TRUE)[1]
      packages <- c("geometry", "gtools")
      install.packages(packages, lib = lib, repos = "https://cloud.r-project.org")
      absent <- packages[!vapply(packages, requireNamespace, logical(1), lib.loc = lib, quietly = TRUE)]
      if (length(absent) > 0) stop("インストールできなかったパッケージ: ", paste(absent, collapse = ", "))
    ' "$work/rlib"
    ;;
  *)
    echo "未対応の OS: $(uname -s) (R・GMP・geometry・gtools を入れてから、下のパッケージの取得以降を実行する)" >&2
    exit 1
    ;;
esac

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
R_LIBS="$work/rlib" Rscript -e 'cat(R.version.string, "\n"); for (p in c("geometry", "gtools", "rcdd", "CoopGame")) cat(p, as.character(packageVersion(p)), "\n")'
echo "export R_LIBS=$work/rlib TUGLAB_DIR=$work/TUGLab/R"
