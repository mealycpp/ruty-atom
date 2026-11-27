#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board, detached: build ruty_apps and the C software baseline, then run the application rows on the three atom images.
#   nohup bash ~/Ruty/apps/run_apps.sh > ~/Ruty/apps/run_apps.log 2>&1 < /dev/null &
cd ~/Ruty/apps || exit 1
echo "START $(date +%H:%M:%S) hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
gcc -c cacheops.S -o cacheops.o && rustc -O -C link-arg=cacheops.o -o ruty_apps ruty_apps.rs 2>&1 | grep -E "^error" -A12 | head -60
[ -x ruty_apps ] || { echo "BUILD FAILED"; exit 1; }
gcc -O3 -o keccak_bench keccak_bench.c 2>&1 | head -5
echo "== software baseline on the ARM (one core, portable C, gcc -O3): $(grep -m1 'model name\|CPU part' /proc/cpuinfo | tr -s ' ')" | tee sw.log
for spec in "shake128 34 840 400" "shake128 34 504 400" "turboshake128 65536 32 20" "shake128 65536 32 20" "turboshake256 232 32 2000" \
            "minishake128 100 16 2000" "minishake256 48 32 2000" "shake256 64 16 2000" "turboshake256 64 16 2000" "minishake128 64 16 2000"; do
  for r in 1 2 3; do taskset -c 1 ./keccak_bench $spec; done | sort -t= -k5 -n | head -1 | tee -a sw.log
done
for spec in "atom_v3_n1_450 1 450" "atom_v3_n2_420 2 420" "atom_v3_n4_400 4 400"; do
  set -- $spec
  echo "== $1"
  sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c \"from pynq import Overlay; Overlay('/home/xilinx/Ruty/$1/ruty.bit'); print('loaded')\"" 2>&1 | tail -1
  sudo -n timeout 900 ./ruty_apps $2 $3 2>&1 | tee apps_$1.log
done
echo "DONE $(date +%H:%M:%S)"
