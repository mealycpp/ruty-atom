#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Fifth and LAST calibration round (settings 33 to 40). Round 4 found one knob that matters: the number of L1D miss buffers.
# 8 buffers: tag copy 0.78, 1 MB copy 0.96 of the board. 4 buffers (ARM's own HPI figure): tag copy 1.02, 1 MB copy 1.77.
# This round tries 4 to 6 buffers with an L2 prefetcher, which shortens the path the 1 MB copy waits on.
R=/mnt/c/Users/elhad/ruty/gem5_atom; cd ~/gem5 || exit 1
B="--cpu hpi --l1pf 2 --l1lat 1"
run() { name=$1; shift
  timeout 3000 build/ARM/gem5.opt -d $R/calib/m5out_$name $R/ruty_se2.py --cmd $R/ruty_cost_aarch64 --options "0 3" "$@" 2>&1 | grep -E 'COST|exit|panic|fatal|Error' > $R/calib/$name.txt
  echo "$(date +%H:%M:%S) done $name: $*"; }
run r5_m4_l2pf2   $B --l1mshr 4 --l2pf 2   &
run r5_m4_l2pf4   $B --l1mshr 4 --l2pf 4   &
run r5_m4_l2pf8   $B --l1mshr 4 --l2pf 8   &
run r5_m5_l2pf2   $B --l1mshr 5 --l2pf 2   &
run r5_m5_l2pf4   $B --l1mshr 5 --l2pf 4   &
run r5_m6         $B --l1mshr 6            &
run r5_m6_l2pf1   $B --l1mshr 6 --l2pf 1   &
run r5_m6_l2pf2   $B --l1mshr 6 --l2pf 2   &
wait; echo ALL_DONE
