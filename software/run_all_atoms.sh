#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board, detached: every test of every atom image that has been copied, one after another.  Log: ~/Ruty/all_atoms.log
#   nohup bash ~/Ruty/run_all_atoms.sh > ~/Ruty/all_atoms.log 2>&1 &
cd ~/Ruty || exit 1
echo "START $(date +%H:%M:%S) hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
for spec in "atom_v3_n1_450 1 450" "atom_v3_n2_420 2 420" "atom_v3_n4_400 4 400" "atom_v1_n6_300 6 300"; do
  set -- $spec
  [ -f ~/Ruty/$1/ruty.bit ] || { echo "== $1: no image, skipped"; continue; }
  cp -n ~/Ruty/ruty_v4_n1_600/ruty_ref.py ~/Ruty/$1/ 2>/dev/null
  bash ~/Ruty/$1/run_atom.sh $1 $2 $3
done
echo "DONE $(date +%H:%M:%S)"
