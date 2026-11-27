#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board, detached: build ruty_tenants and run the two-tenant experiment on the 2-atom and 4-atom images.
#   nohup bash ~/Ruty/apps/run_tenants.sh > ~/Ruty/apps/run_tenants.log 2>&1 < /dev/null &
cd ~/Ruty/apps || exit 1
echo "START $(date +%H:%M:%S) hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
rm -f ruty_tenants
rustc -O -C link-arg=cacheops.o -o ruty_tenants ruty_tenants.rs 2>&1 | grep -E "^(error|warning: unused)" -A12 | head -80
[ -x ruty_tenants ] || { echo "BUILD FAILED"; exit 1; }
for spec in "atom_v3_n2_420 2 420" "atom_v3_n4_400 4 400"; do
  set -- $spec
  echo "== $1"
  sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c \"from pynq import Overlay; Overlay('/home/xilinx/Ruty/$1/ruty.bit'); print('loaded')\"" 2>&1 | tail -1
  sudo -n timeout 600 ./ruty_tenants $2 $3 2>&1 | tee tenants_$1.log
done
echo "DONE $(date +%H:%M:%S)"
