#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board, detached: build ruty_sign and run the full SLH-DSA-SHAKE-128f application (key generation, signing, verification,
# and the firmware-authentication application) on the 1-, 2- and 4-atom v3 images.
#   nohup bash ~/Ruty/apps/run_sign.sh "std" 25 > ~/Ruty/apps/run_sign_std.log 2>&1 < /dev/null &
# Modes: std, fw (FIPS 205) and turbo, mini (NON-STANDARD, not FIPS 205). One log per mode and image: sign_<mode>_<image>.log
cd ~/Ruty/apps || exit 1
MODES=${1:-std}; REPS=${2:-25}
sed -i 's/\r$//' ruty_sign.rs
echo "START $(date +%H:%M:%S) modes=$MODES reps=$REPS hapi units active: $(systemctl list-units --no-pager --plain --state=active 'hapi*' | grep -c '^hapi-')"
if [ ! -x ruty_sign ] || [ ruty_sign.rs -nt ruty_sign ]; then
  rm -f ruty_sign
  rustc -O -C link-arg=cacheops.o -o ruty_sign ruty_sign.rs 2>&1 | grep -E "^error" -A14 | head -90
fi
[ -x ruty_sign ] || { echo "BUILD FAILED"; exit 1; }
sha256sum ruty_sign.rs
for spec in "atom_v3_n1_450 1 450" "atom_v3_n2_420 2 420" "atom_v3_n4_400 4 400"; do
  set -- $spec
  echo "== $1 $(date +%H:%M:%S)"
  sudo -n bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c \"from pynq import Overlay; Overlay('/home/xilinx/Ruty/$1/ruty.bit'); print('loaded')\"" 2>&1 | tail -1
  for m in $MODES; do
    sudo -n timeout 1500 ./ruty_sign $2 $3 $m $REPS > sign_${m}_$1.log 2>&1
    echo "-- mode $m exit=$?"; grep -E "RESULT (check|summary|error|time|timing)" sign_${m}_$1.log | cut -c1-230
  done
done
echo "DONE $(date +%H:%M:%S)"
