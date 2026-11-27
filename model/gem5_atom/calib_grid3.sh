#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Third calibration round. Round 2: a degree-2 stride prefetcher on L1 only puts the memory copies within 2% to 19% of the board;
# what remains is the core itself (compute routines 1.2x to 1.4x slow, tight load loop 2x slow). This round varies the core.
R=/mnt/c/Users/elhad/ruty/gem5_atom; cd ~/gem5 || exit 1
run() { name=$1; shift
  timeout 3000 build/ARM/gem5.opt -d $R/calib/m5out_$name $R/ruty_se2.py --cmd $R/ruty_cost_aarch64 --options "0 3" "$@" 2>&1 | grep -E 'COST|exit|panic|fatal' > $R/calib/$name.txt
  echo "$(date +%H:%M:%S) done $name: $*"; }
run r3_hpi_pf2          --cpu hpi   --l1pf 2                 &
run r3_hpi_pf2_l1lat1   --cpu hpi   --l1pf 2 --l1lat 1       &
run r3_minor_pf2_l1lat1 --cpu minor --l1pf 2 --l1lat 1       &
run r3_hpi_pf3          --cpu hpi   --l1pf 3                 &
run r3_hpi_pf2_l2lat9   --cpu hpi   --l1pf 2 --l2lat 9       &
run r3_minor_pf3_l1lat1 --cpu minor --l1pf 3 --l1lat 1       &
run r3_hpi_pf2_l2pf1    --cpu hpi   --l1pf 2 --l2pf 1        &
run r3_hpi_pf2_wb16     --cpu hpi   --l1pf 2 --wb 16         &
wait; echo ALL_DONE
