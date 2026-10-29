#!/usr/bin/env bash
set -Eeuo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
test_runtime="$(mktemp -d -t dotfiles-tests.XXXXXXXX)"
trap 'rm -rf -- "$test_runtime"' EXIT
export XDG_RUNTIME_DIR="$test_runtime" PYTHONDONTWRITEBYTECODE=1
python3 scripts/check.py
python3 -m unittest discover -s tests -v
for profile in desktop laptop; do
    cc -O2 -Wall -Wextra -Werror -fsyntax-only "$profile/eww/control-center/scripts/fast-client.c"
    python3 -m unittest discover -s "$profile/eww/control-center/tests" -v
    python3 -m unittest discover -s "$profile/eww/desktop/sysmon/scripts" -p 'test_*.py' -v
done
python3 -m unittest discover -s desktop/waybar/scripts -p 'test_*.py' -v
read -r -a gtk_flags <<< "$(pkg-config --cflags --libs gtk+-3.0 json-glib-1.0)"
cc -O2 -Wall -Wextra -Werror desktop/waybar/cffi/test-workspace-animation.c \
    -o "$test_runtime/test-workspace-animation" "${gtk_flags[@]}" -lm
"$test_runtime/test-workspace-animation"
