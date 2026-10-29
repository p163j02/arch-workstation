#!/bin/bash
# ram-info.sh — Uso de RAM y swap desde /proc/meminfo
# Salida: JSON con valores en GB y porcentaje

mem_total=$(    grep "^MemTotal:"     /proc/meminfo | awk '{print $2}')
mem_free=$(     grep "^MemFree:"      /proc/meminfo | awk '{print $2}')
mem_buffers=$(  grep "^Buffers:"      /proc/meminfo | awk '{print $2}')
mem_cached=$(   grep "^Cached:"       /proc/meminfo | awk '{print $2}')
mem_sreclam=$(  grep "^SReclaimable:" /proc/meminfo | awk '{print $2}')
swap_total=$(   grep "^SwapTotal:"    /proc/meminfo | awk '{print $2}')
swap_free=$(    grep "^SwapFree:"     /proc/meminfo | awk '{print $2}')

# RAM usada real = Total - Free - Buffers - Cached - SReclaimable
mem_used=$(( mem_total - mem_free - ${mem_buffers:-0} - ${mem_cached:-0} - ${mem_sreclam:-0} ))
swap_used=$(( swap_total - swap_free ))

# Convertir a GB con 1 decimal
ram_used_gb=$( awk "BEGIN{ printf \"%.1f\", $mem_used  / 1048576 }")
ram_total_gb=$(awk "BEGIN{ printf \"%.1f\", $mem_total  / 1048576 }")
swap_used_gb=$(awk "BEGIN{ printf \"%.1f\", $swap_used  / 1048576 }")
swap_total_gb=$(awk "BEGIN{ printf \"%.1f\", $swap_total / 1048576 }")

# Porcentajes
ram_pct=$( awk "BEGIN{ printf \"%d\", ($mem_used  / $mem_total)  * 100 }")
if [ "${swap_total:-0}" -gt 0 ]; then
  swap_pct=$(awk "BEGIN{ printf \"%d\", ($swap_used / $swap_total) * 100 }")
else
  swap_pct=0
  swap_total_gb="0.0"
  swap_used_gb="0.0"
fi

echo "{\"ram_used_gb\":\"$ram_used_gb\",\"ram_total_gb\":\"$ram_total_gb\",\"ram_pct\":$ram_pct,\"swap_used_gb\":\"$swap_used_gb\",\"swap_total_gb\":\"$swap_total_gb\",\"swap_pct\":$swap_pct}"
