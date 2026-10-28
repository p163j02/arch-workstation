#!/usr/bin/env bash
set -Eeuo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
test_runtime="$(mktemp -d -t dotfiles-tests.XXXXXXXX)"
trap 'rm -rf -- "$test_runtime"' EXIT
export XDG_RUNTIME_DIR="$test_runtime" PYTHONDONTWRITEBYTECODE=1
python3 scripts/check.py
cc -O2 -Wall -Wextra -Werror -fsyntax-only shared/eww/control-center/scripts/fast-client.c
python3 -m unittest discover -s tests -v
python3 -m unittest discover -s shared/eww/control-center/tests -v
python3 -m unittest discover -s shared/eww/desktop/sysmon/scripts -p 'test_*.py' -v
