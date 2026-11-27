#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board, detached: build ruty_dfg and run the job-graph scheduler experiment on the 4-atom and 2-atom images.
#   nohup bash ~/Ruty/apps/run_dfg.sh > ~/Ruty/apps/run_dfg.log 2>&1 < /dev/null &
cd ~/Ruty/apps || exit 1
echo "START $(date +%H:%M:%S) hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
rm -f ruty_dfg
rustc -O -C link-arg=cacheops.o -o ruty_dfg ruty_dfg.rs 2>&1 | grep -E "^error" -A14 | head -90
[ -x ruty_dfg ] || { echo "BUILD FAILED"; exit 1; }
for spec in "atom_v3_n4_400 4 400" "atom_v3_n2_420 2 420"; do
  set -- $spec
  echo "== $1"
  sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c \"from pynq import Overlay; Overlay('/home/xilinx/Ruty/$1/ruty.bit'); print('loaded')\"" 2>&1 | tail -1
  sudo -n timeout 900 ./ruty_dfg $2 $3 2>&1 | tee dfg_$1.log
done
echo "DONE $(date +%H:%M:%S)"
