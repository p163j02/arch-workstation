#!/bin/bash
# Returns disk info for the root partition in GiB with filesystem type
read -r used_bytes total_bytes pct fstype <<< "$(df -B1 --output=used,size,pcent,fstype / | tail -1 | tr -s ' ' | sed 's/%//')"
used_gib=$(awk "BEGIN {printf \"%.2f\", $used_bytes / 1073741824}")
total_gib=$(awk "BEGIN {printf \"%.2f\", $total_bytes / 1073741824}")
printf '{"used_gib":"%s","total_gib":"%s","pct":%d,"fstype":"%s"}' \
  "$used_gib" "$total_gib" "$pct" "$fstype"
