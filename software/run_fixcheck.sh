#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board, detached: build ruty_fixcheck and run it on every image listed in fixcheck_specs.txt (one line: <folder under ~/Ruty> <rt|w|n> <engine MHz>).
# The 4-atom image is loaded again when the runs are over, so the board is left as it was found.
#   nohup bash ~/Ruty/apps/run_fixcheck.sh > ~/Ruty/apps/run_fixcheck.log 2>&1 < /dev/null &
cd ~/Ruty/apps || exit 1
echo "START $(date +%H:%M:%S) hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
rm -f ruty_fixcheck
rustc -O -C link-arg=cacheops.o -o ruty_fixcheck ruty_fixcheck.rs 2>&1 | grep -E "^error" -A12 | head -80
[ -x ruty_fixcheck ] || { echo "BUILD FAILED"; exit 1; }
load() {
  sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c \"from pynq import Overlay; Overlay('/home/xilinx/Ruty/$1/ruty.bit'); print('loaded')\"" 2>&1 | tail -1
}
while read -r img mode mhz; do
  [ -n "$img" ] || continue
  echo "== $img mode=$mode engine=$mhz md5=$(md5sum < /home/xilinx/Ruty/$img/ruty.bit | cut -c1-12)"
  [ -f /home/xilinx/Ruty/$img/ruty.bit ] || { echo "NO IMAGE $img"; continue; }
  load $img
  sudo -n timeout 600 ./ruty_fixcheck $mode $mhz $img 2>&1 | tee fixcheck_$img.log
done < fixcheck_specs.txt
echo "== restore atom_v3_n4_400"
load atom_v3_n4_400
echo "DONE $(date +%H:%M:%S)"
