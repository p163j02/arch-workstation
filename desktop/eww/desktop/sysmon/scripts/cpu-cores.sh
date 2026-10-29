#!/bin/bash
# cpu-cores.sh — Uso de CPU por núcleo
# Salida: JSON plano {"c0":4,"c1":2,...} (eww no soporta arrays por índice)

read_stat() { grep "^cpu[0-9]" /proc/stat; }

BEFORE=$(read_stat)
sleep 0.5
AFTER=$(read_stat)

result="{"
sep=""

for i in $(seq 0 7); do
  before_line=$(echo "$BEFORE" | grep "^cpu${i} ")
  after_line=$(echo "$AFTER"  | grep "^cpu${i} ")

  if [ -z "$before_line" ] || [ -z "$after_line" ]; then
    result="${result}${sep}\"c${i}\":0"
    sep=","
    continue
  fi

  read -r _ b_user b_nice b_sys b_idle b_iowait b_irq b_softirq _ <<< "$before_line"
  read -r _ a_user a_nice a_sys a_idle a_iowait a_irq a_softirq _ <<< "$after_line"

  b_total=$(( b_user + b_nice + b_sys + b_idle + ${b_iowait:-0} + ${b_irq:-0} + ${b_softirq:-0} ))
  a_total=$(( a_user + a_nice + a_sys + a_idle + ${a_iowait:-0} + ${a_irq:-0} + ${a_softirq:-0} ))
  delta_total=$(( a_total - b_total ))
  delta_idle=$(( a_idle - b_idle ))

  if [ "$delta_total" -gt 0 ]; then
    pct=$(awk "BEGIN{ printf \"%d\", (($delta_total - $delta_idle) / $delta_total) * 100 }")
    [ "$pct" -lt 0 ]   && pct=0
    [ "$pct" -gt 100 ] && pct=100
  else
    pct=0
  fi

  result="${result}${sep}\"c${i}\":${pct}"
  sep=","
done

echo "${result}}"
