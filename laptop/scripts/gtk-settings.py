#!/usr/bin/env python3
"""Apply the selected profile's desktop appearance through GSettings."""
import configparser
from pathlib import Path
import subprocess
import sys


def apply(path):
    config = configparser.ConfigParser()
    with Path(path).open() as source:
        config.read_file(source)
    for schema in config.sections():
        for key, value in config[schema].items():
            subprocess.run(['gsettings', 'set', schema, key, value], check=True)


if __name__ == '__main__':
    apply(sys.argv[1])
