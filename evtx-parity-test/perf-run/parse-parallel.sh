#!/usr/bin/env bash
# parse-parallel.sh — Gustafson 式文件级并行跑批（R38 配套，G0 语义无改动）。
#
# 单文件的 Amdahl 上限 ≈1.7×（R38 后 f≈0.59，剩余串行在 evtx crate JSON 树构建内）；
# 多文件跑批时按"一文件一进程"扇出，总吞吐随核数线性扩展。适用于
# 大量同量级文件的采集机/取证跑批场景；单一巨型文件占主导时收益受限于该文件本身。
#
# 用法: parse-parallel.sh <evtx-dir|file.evtx...> <outdir> [并发进程数=4] [每进程 --threads=2]
# 环境: MSG_DB=... 必填；LANG_CODE 可选（默认 zh-CN）
#
# 例: MSG_DB=../msgdb/hotech-msg-*.db ./parse-parallel.sh ../big-evtx out 4 2
set -euo pipefail

MSG_DB="${MSG_DB:?MSG_DB=<messages.db> 必填}"
LANG_CODE="${LANG_CODE:-zh-CN}"
EXE="$(dirname "$0")/../tools/evtx-tools.exe"

inputs=()
if [ $# -ge 1 ] && [ -d "$1" ]; then
  while IFS= read -r f; do inputs+=("$f"); done < <(find "$1" -maxdepth 1 -iname '*.evtx' | sort)
  shift
else
  while [ $# -ge 1 ] && [ -f "$1" ]; do inputs+=("$1"); shift; done
fi
[ ${#inputs[@]} -gt 0 ] || { echo "no .evtx inputs" >&2; exit 2; }
outdir="${1:?outdir required}"; shift
workers="${1:-4}"; shift || true
threads="${1:-2}"
mkdir -p "$outdir"

t0=$(date +%s%N)
pids=(); names=()
for f in "${inputs[@]}"; do
  stem="$(basename "$f")"; stem="${stem%.[eE][vV][tT][xX]}"
  while [ "$(jobs -rp | wc -l)" -ge "$workers" ]; do sleep 0.2; done
  echo "[spawn] $stem (threads=$threads)"
  "$EXE" parse --format csv --msg-db "$MSG_DB" --lang "$LANG_CODE" \
    --threads "$threads" -o "$outdir/$stem.csv" "$f" \
    > "$outdir/$stem.log" 2>&1 &
  pids+=($!); names+=("$stem")
done
fail=0
for i in "${!pids[@]}"; do
  if ! wait "${pids[$i]}"; then echo "[FAIL] ${names[$i]}" >&2; fail=1; fi
done
t1=$(date +%s%N)
echo "[done] ${#inputs[@]} files, workers=$workers x threads=$threads, wall=$(( (t1-t0)/1000000 )) ms"
exit $fail
