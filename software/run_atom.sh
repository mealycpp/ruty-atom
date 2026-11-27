#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board: every test of one atom image.   usage: run_atom.sh <dir under ~/Ruty> <atoms> <engine MHz>
cd ~/Ruty/$1 || exit 1
sed -i 's/\r$//' *.sh *.rs *.S *.py 2>/dev/null
echo "== $1: hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; timeout 600 python3 ruty_chain.py ruty.bit" > chain.log 2>&1
grep -E "id=|fips205|chain_time|summary" chain.log
sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; timeout 900 python3 ruty_cluster.py ruty.bit $2 $3" > cluster.log 2>&1
echo "clusters correct: $(grep -c 'mismatches=0' cluster.log)"; tail -1 cluster.log; grep -E "atoms=$2 split=(0|$2) " cluster.log
gcc -c cacheops.S -o cacheops.o && rustc -O -C link-arg=cacheops.o -o ruty_sched ruty_sched.rs 2>&1 | grep -E "^error" -A8 | head -20
sudo -n timeout 300 ./ruty_sched $2 $3 > sched.log 2>&1; grep -E "sched2" sched.log
