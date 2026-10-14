#!/usr/bin/env bash
# 比較対象のうち、R 以外の無料の OSS (MatTuGames を Octave で、pyCoopGame を Python で) を用意する
# (macOS 26、Homebrew で確認)。
# MatTuGames は MATLAB 用で、Octave では標準の linprog がないため、octave/linprog.m (GLPK) を代わりに使う。
# 比較に使った版を再現するため、各リポジトリはコミットを固定して取得する。
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="${WORK:-$here/work}"
mkdir -p "$work"

# 比較時の版: octave 11.3.0, glpk 5.0
brew install octave glpk

# リポジトリ コミット (版)
pins=(
  "MatTuGames https://github.com/himeinhardt/MatTuGames 74b8ec71832e3c63e6d4c59b0019ea0e1d9608ac"  # 1.9.2.1
  "pyCoopGame https://github.com/flechtenberg/pyCoopGame 72153c7771bc6d910be023e5d72c8922cecaa80f"  # 0.0.6 以降
)
for pin in "${pins[@]}"; do
  read -r name url commit <<< "$pin"
  if [ ! -d "$work/$name" ]; then
    git init -q "$work/$name"
    git -C "$work/$name" remote add origin "$url"
  fi
  git -C "$work/$name" fetch -q --depth 1 origin "$commit"
  git -C "$work/$name" checkout -q --detach "$commit"
done

# pyCoopGame は Python 3.12 以下を要求する。比較時の版: pyomo 6.10.1, pandas 3.0.6
uv venv -q -p 3.12 "$work/pyenv"
VIRTUAL_ENV="$work/pyenv" uv pip install -q pyomo==6.10.1 pandas==3.0.6
echo "export MATTUGAMES_DIR=$work/MatTuGames/mat_tugames PYCOOPGAME_DIR=$work/pyCoopGame PYCOOPGAME_PYTHON=$work/pyenv/bin/python"
