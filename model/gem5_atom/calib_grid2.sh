#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Second calibration round. Round 1: no prefetcher copies memory 3.5x too slowly, degree-8 prefetchers 1.8x too fast. The board lies between.
R=/mnt/c/Users/elhad/ruty/gem5_atom; cd ~/gem5 || exit 1
run() { name=$1; shift
  timeout 3000 build/ARM/gem5.opt -d $R/calib/m5out_$name $R/ruty_se2.py --cmd $R/ruty_cost_aarch64 --options "0 3" "$@" 2>&1 | grep -E 'COST|exit|panic|fatal' > $R/calib/$name.txt
  echo "$(date +%H:%M:%S) done $name: $*"; }
run r2_pf1          --l1pf 1 --l2pf 1                              &
run r2_pf2_l1only   --l1pf 2                                       &
run r2_pf2          --l1pf 2 --l2pf 2                              &
run r2_pf2_16x4     --l1pf 2 --l2pf 2 --mem DDR4_2400_16x4         &
run r2_pf4_16x4     --l1pf 4 --l2pf 4 --mem DDR4_2400_16x4         &
run r2_pf3_l2lat9   --l1pf 3 --l2lat 9                             &
run r2_pf4_simple60 --l1pf 4 --l2pf 4 --mem simple --memlat 60ns   &
run r2_pf2_l2pf4    --l1pf 2 --l2pf 4                              &
wait; echo ALL_DONE
