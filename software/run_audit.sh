#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# run_audit.sh (NEW, 2026-11-26): the two runs the audit script reproduce/check_claims.py found missing. On the board, detached:
#   nohup bash ~/Ruty/apps/run_audit.sh > ~/Ruty/apps/run_audit.log 2>&1 < /dev/null &
# 1. the NIST and RFC 9861 known-answer tests (ruty_kat3.py) on atom 0 of the 1-, 2- and 4-atom v3 images;
# 2. Table I again with the individual run times logged (ruty_sign5, ruty_modes2: ruty_sign4 and ruty_modes plus one printed field);
# 3. the known-answer tests on the remaining atoms of the 2- and 4-atom images.
# It ends by loading the four-atom image, as the other run scripts do.
cd ~/Ruty/apps || exit 1
REPS=${1:-25}
sed -i 's/\r$//' ruty_sign5.rs ruty_modes2.rs ruty_kat3.py
echo "START $(date '+%F %H:%M:%S') reps=$REPS hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
OTHER=$(pgrep -fa 'ruty_(sign|apps|dfg|tenants|cost|fixcheck|sched|modes)|modes_bench|keccak_bench|ruty_kat' | grep -v pgrep)
if [ -n "$OTHER" ]; then echo "OTHER RUTY PROGRAM RUNNING, STOP: $OTHER"; exit 1; fi
PY='source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh;'
load() { sudo -n bash -c "$PY python3 -c \"from pynq import Overlay; Overlay('/home/xilinx/Ruty/$1/ruty.bit'); print('loaded')\"" 2>&1 | tail -1; }
kat() { # <image> <atom>
  echo "== kat $1 atom $2 $(date +%H:%M:%S)"
  ( cd ~/Ruty/atom_v1_n1_350 && sudo -n bash -c "$PY timeout 3000 python3 /home/xilinx/Ruty/apps/ruty_kat3.py /home/xilinx/Ruty/$1/ruty.bit kat $2" 2>&1 | grep -E "RESULT|Error|error|Traceback|line " > ~/Ruty/apps/kat3_$1_atom$2.log )
  tail -1 ~/Ruty/apps/kat3_$1_atom$2.log | cut -c1-200
}
sha256sum ruty_kat3.py ~/Ruty/atom_v1_n1_350/ruty_ref.py ~/Ruty/atom_v1_n1_350/kat/*.rsp ~/Ruty/atom_v1_n1_350/kat/rfc9861.txt | cut -c1-110
kat atom_v3_n1_450 0; kat atom_v3_n2_420 0; kat atom_v3_n4_400 0

rm -f ruty_sign5 ruty_modes2
rustc -O -C link-arg=cacheops.o -o ruty_sign5 ruty_sign5.rs 2>&1 | grep -E "^error" -A14 | head -60
rustc -O -C link-arg=cacheops.o -o ruty_modes2 ruty_modes2.rs 2>&1 | grep -E "^error" -A14 | head -60
if [ -x ruty_sign5 ] && [ -x ruty_modes2 ]; then
  sha256sum ruty_sign5.rs ruty_modes2.rs
  for spec in "atom_v3_n1_450 1 450" "atom_v3_n2_420 2 420" "atom_v3_n4_400 4 400"; do
    set -- $spec
    echo "== timing $1 $(date +%H:%M:%S)"; load $1
    for m in std turbo mini fw; do
      sudo -n timeout 1500 ./ruty_sign5 $2 $3 $m $REPS word > sign5_${m}_word_$1.log 2>&1
      echo "-- sign5 $m exit=$?"; grep -E "RESULT (summary|error)" sign5_${m}_word_$1.log | cut -c1-200
    done
    sudo -n timeout 1800 ./ruty_modes2 $2 $3 kt $REPS > modes2_kt_$1.log 2>&1; echo "-- kt exit=$?"; grep -E "summary|error|FAIL" modes2_kt_$1.log | cut -c1-200
    sudo -n timeout 2400 ./ruty_modes2 $2 $3 merkle $REPS 16 > modes2_merkle_$1.log 2>&1; echo "-- merkle exit=$?"; grep -E "summary|error|FAIL" modes2_merkle_$1.log | cut -c1-200
  done
else
  echo "BUILD FAILED: timing part skipped"
fi

kat atom_v3_n2_420 1; kat atom_v3_n4_400 1; kat atom_v3_n4_400 2; kat atom_v3_n4_400 3
echo "== reload four-atom image $(date +%H:%M:%S)"; load atom_v3_n4_400
echo "ALL DONE $(date '+%F %H:%M:%S')"
