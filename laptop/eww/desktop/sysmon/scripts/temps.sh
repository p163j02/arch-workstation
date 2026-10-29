#!/bin/bash
# temps.sh — Temperaturas de CPU y GPU desde /sys/class/hwmon
# Incluye cpu_color y gpu_color para evitar lógica condicional en eww.yuck

cpu_temp=0
gpu_temp=0

for hwmon_dir in /sys/class/hwmon/hwmon*/; do
  name=$(cat "${hwmon_dir}name" 2>/dev/null)
  case "$name" in
    k10temp|coretemp)
      if [ -f "${hwmon_dir}temp1_input" ]; then
        val=$(cat "${hwmon_dir}temp1_input" 2>/dev/null)
        cpu_temp=$(awk "BEGIN{ printf \"%d\", $val / 1000 }")
      fi
      ;;
    amdgpu)
      if [ -f "${hwmon_dir}temp1_input" ]; then
        val=$(cat "${hwmon_dir}temp1_input" 2>/dev/null)
        gpu_temp=$(awk "BEGIN{ printf \"%d\", $val / 1000 }")
      fi
      ;;
  esac
done

color_for() {
  local t=$1
  if   [ "$t" -gt 85 ]; then echo "#f7768e"
  elif [ "$t" -gt 70 ]; then echo "#e0af68"
  else echo "#9ece6a"
  fi
}

CPU_COLOR=$(color_for $cpu_temp)
GPU_COLOR=$(color_for $gpu_temp)

echo "{\"cpu\":$cpu_temp,\"gpu\":$gpu_temp,\"cpu_color\":\"$CPU_COLOR\",\"gpu_color\":\"$GPU_COLOR\"}"
