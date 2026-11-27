#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# run_modes.sh (NEW): native applications of the RUTY-ATOM modes. On the board, detached:
#   nohup bash ~/Ruty/apps/run_modes.sh 25 12,16,20 > ~/Ruty/apps/run_modes.log 2>&1 < /dev/null &
# Builds ruty_modes (atoms) and modes_bench (A53 software), runs the software baseline, then KT128 and the Merkle trees on the 1-, 2- and 4-atom v3 images.
cd ~/Ruty/apps || exit 1
REPS=${1:-25}; KS=${2:-12,16,20}
sed -i 's/\r$//' ruty_modes.rs modes_bench.c
echo "START $(date +%H:%M:%S) reps=$REPS ks=$KS hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
OTHER=$(pgrep -fa 'ruty_(sign|apps|dfg|tenants|cost|fixcheck|sched)|keccak_bench' | grep -v pgrep)
if [ -n "$OTHER" ]; then echo "OTHER RUTY PROGRAM RUNNING, STOP: $OTHER"; exit 1; fi
rm -f ruty_modes modes_bench
rustc -O -C link-arg=cacheops.o -o ruty_modes ruty_modes.rs 2>&1 | grep -E "^error" -A14 | head -60
gcc -O3 -mcpu=cortex-a53 -pthread -o modes_bench modes_bench.c 2>&1 | head -10
[ -x ruty_modes ] && [ -x modes_bench ] || { echo "BUILD FAILED"; exit 1; }
sha256sum ruty_modes.rs modes_bench.c
echo "== software baseline $(date +%H:%M:%S): $(grep -m1 'CPU part' /proc/cpuinfo | tr -s ' ') x$(nproc), gcc -O3 -mcpu=cortex-a53"
{ ./modes_bench vectors 1; ./modes_bench vectors 4; ./modes_bench kt $REPS 1; ./modes_bench kt $REPS 4; ./modes_bench merkle $REPS 1 $KS; ./modes_bench merkle $REPS 4 $KS; } > modes_sw.log 2>&1
grep -E "summary|swkt|swflat" modes_sw.log | cut -c1-200
for spec in "atom_v3_n1_450 1 450" "atom_v3_n2_420 2 420" "atom_v3_n4_400 4 400"; do
  set -- $spec
  echo "== $1 $(date +%H:%M:%S)"
  sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c \"from pynq import Overlay; Overlay('/home/xilinx/Ruty/$1/ruty.bit'); print('loaded')\"" 2>&1 | tail -1
  sudo -n timeout 1800 ./ruty_modes $2 $3 kt $REPS > modes_kt_$1.log 2>&1; echo "-- kt exit=$?"; grep -E "summary|error|FAIL" modes_kt_$1.log | cut -c1-200
  sudo -n timeout 2400 ./ruty_modes $2 $3 merkle $REPS $KS > modes_merkle_$1.log 2>&1; echo "-- merkle exit=$?"; grep -E "summary|error|FAIL" modes_merkle_$1.log | cut -c1-200
done
echo "DONE $(date +%H:%M:%S)"
