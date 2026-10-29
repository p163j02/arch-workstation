#!/usr/bin/env python3
"""Build a uniquely named adapter so Waybar reloads cannot reuse an old dlopen."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--install-directory', type=Path,
                        help='Final library directory when building in an installation stage')
    args = parser.parse_args()
    directory = Path(__file__).resolve().parent
    config = directory.parent / "config.jsonc"
    flags = shlex.split(subprocess.check_output(
        ["pkg-config", "--cflags", "--libs", "gtk+-3.0", "json-glib-1.0"], text=True
    ))
    with tempfile.NamedTemporaryFile(dir=directory, prefix=".animation-", suffix=".so", delete=False) as file:
        temporary = Path(file.name)
    try:
        subprocess.run(["gcc", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror",
                        str(directory / "workspace-animation.c"), "-o", str(temporary), *flags, "-lm"], check=True)
        digest = hashlib.sha256(temporary.read_bytes()).hexdigest()[:16]
        library = directory / f"workspace-animation-{digest}.so"
        if library.exists():
            temporary.unlink()
        else:
            temporary.chmod(0o755)
            temporary.replace(library)
        original = config.read_text()
        module_path = (args.install_directory / library.name) if args.install_directory else library
        updated, count = re.subn(
            r'("cffi/workspace-animation"\s*:\s*\{\s*"module_path"\s*:\s*)"[^"]+"',
            lambda match: match[1] + json.dumps(str(module_path)), original,
        )
        if count != 1:
            raise ValueError("No se encontró el module_path de cffi/workspace-animation")
        if updated != original:
            target = config.with_name(config.name + ".tmp")
            target.write_text(updated)
            target.replace(config)
        print(library)
    finally:
        temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
