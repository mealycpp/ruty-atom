#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board: form every cluster of the N-atom image and measure it.  usage: run_cluster.sh <dir under ~/Ruty> <N> <engine MHz>
cd ~/Ruty/$1 || exit 1
sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; timeout 900 python3 ruty_cluster.py ruty.bit $2 $3" > cluster.log 2>&1
tail -1 cluster.log; echo "correct clusters: $(grep -c 'mismatches=0' cluster.log)"; grep -E "atoms=$2 split=(0|$(($2/2))|$2) " cluster.log
