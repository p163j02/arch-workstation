#!/usr/bin/env python3
import subprocess, json, os

def get_procs(sort_key):
    result = subprocess.run(
        ["ps", "aux", f"--sort={sort_key}"],
        capture_output=True, text=True
    )
    lines = result.stdout.strip().split("\n")[1:6]  # skip header, top 5
    return lines

def parse_cpu(line):
    parts = line.split(None, 10)
    name = os.path.basename(parts[10].split()[0])[:17] if len(parts) > 10 else "---"
    pct  = parts[2] if len(parts) > 2 else "0.0"
    return name, pct

def parse_ram(line):
    parts = line.split(None, 10)
    name   = os.path.basename(parts[10].split()[0])[:17] if len(parts) > 10 else "---"
    rss_kb = int(parts[5]) if len(parts) > 5 else 0
    mb     = rss_kb // 1024
    return name, mb

out = {}

cpu_lines = get_procs("-%cpu")
for i in range(5):
    if i < len(cpu_lines) and cpu_lines[i].strip():
        name, pct = parse_cpu(cpu_lines[i])
    else:
        name, pct = "---", "0.0"
    out[f"cpu{i}_name"] = name
    out[f"cpu{i}_pct"]  = pct

ram_lines = get_procs("-%mem")
for i in range(5):
    if i < len(ram_lines) and ram_lines[i].strip():
        name, mb = parse_ram(ram_lines[i])
    else:
        name, mb = "---", 0
    out[f"ram{i}_name"] = name
    out[f"ram{i}_mb"]   = mb

print(json.dumps(out))
