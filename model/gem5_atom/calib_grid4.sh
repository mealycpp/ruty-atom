#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Fourth calibration round, around the round-3 best (HPI core, degree-2 stride prefetcher on L1D, 1-cycle L1).
# What is left: memory copies 4% to 22% too fast (tag copy worst), small compute routines 5% to 30% too slow.
# This round varies the miss path only: L2 latency, response latencies, miss buffers, prefetcher timing; and ARM's own HPI cache figures.
R=/mnt/c/Users/elhad/ruty/gem5_atom; cd ~/gem5 || exit 1
B="--cpu hpi --l1pf 2 --l1lat 1"
run() { name=$1; shift
  timeout 3000 build/ARM/gem5.opt -d $R/calib/m5out_$name $R/ruty_se2.py --cmd $R/ruty_cost_aarch64 --options "0 3" "$@" 2>&1 | grep -E 'COST|exit|panic|fatal|Error' > $R/calib/$name.txt
  echo "$(date +%H:%M:%S) done $name: $*"; }
run r4_l2lat14        $B --l2lat 14                                                   &
run r4_l2resp5        $B --l2resp 5                                                   &
run r4_hpicache       $B --l2lat 13 --l2resp 5 --l1mshr 4 --l2mshr 4 --wb 4 --pfq 4   &
run r4_l1mshr4        $B --l1mshr 4                                                   &
run r4_pflat4         $B --pflat 4                                                    &
run r4_pfmiss         $B --pfmiss 1                                                   &
run r4_l1resp2        $B --l1resp 2                                                   &
run r4_l2lat14_r5_m4  $B --l2lat 14 --l2resp 5 --l1mshr 4                             &
wait; echo ALL_DONE
