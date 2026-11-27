#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Show the gem5 calibration runs next to the board (real Cortex-A53) numbers. mean_ns per routine.
cd /mnt/c/Users/elhad/ruty/gem5_atom
cat calib_grid.log 2>/dev/null | tail -12
echo "running gem5: $(pgrep -c gem5.opt)"
row() { awk -v n="$1" 'BEGIN{printf "%-16s", n} /COST [a-z]/{split($4,a,"="); printf "%9s", a[2]} END{print ""}' "$2"; }
printf "%-16s" "routine:"; awk '/COST [a-z]/{printf "%9s", substr($2,1,8)} END{print ""}' ../results/cost_native_a53_aup1.txt
row "BOARD A53" ../results/cost_native_a53_aup1.txt
for f in calib/*.txt; do n=$(basename $f .txt); if grep -q "COST end" $f; then row "$n" $f; else echo "$n: not finished or failed: $(grep -E 'panic|fatal|Error' $f | head -1 | cut -c1-100)"; fi; done
