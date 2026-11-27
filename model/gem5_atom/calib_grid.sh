#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Calibrate the gem5 processor model against the real Cortex-A53: run the same ruty_cost binary under several model settings.
# Board reference: results/cost_native_a53_aup1.txt.   nohup bash calib_grid.sh > calib_grid.log 2>&1 &
R=/mnt/c/Users/elhad/ruty/gem5_atom; cd ~/gem5 || exit 1
mkdir -p $R/calib
run() { name=$1; shift
  timeout 3000 build/ARM/gem5.opt -d $R/calib/m5out_$name $R/ruty_se2.py --cmd $R/ruty_cost_aarch64 --options "0 3" "$@" 2>&1 | grep -E 'COST|exit|panic|fatal' > $R/calib/$name.txt
  echo "$(date +%H:%M:%S) done $name: $*"; }
run base                                                         &
run pf_l1_4        --l1pf 4                                       &
run pf_l1_8_l2_8   --l1pf 8 --l2pf 8                              &
run pf8_l2lat6     --l1pf 8 --l2pf 8 --l2lat 6                    &
run pf8_l2lat6_13  --l1pf 8 --l2pf 8 --l2lat 6 --clock 1.333GHz   &
run pf8_simple20   --l1pf 8 --l2pf 8 --l2lat 6 --mem simple --memlat 20ns &
run hpi_pf8        --cpu hpi --l1pf 8 --l2pf 8 --l2lat 6          &
run o3_pf8         --cpu o3  --l1pf 8 --l2pf 8 --l2lat 6          &
wait; echo ALL_DONE
