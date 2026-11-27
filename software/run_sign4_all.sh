#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board, detached: every mode of ruty_sign4 on the 1-, 2- and 4-atom images, then the two-tenant run on the 4-atom image.
#   nohup bash ~/Ruty/apps/run_sign4_all.sh > ~/Ruty/apps/run_sign4_all.log 2>&1 < /dev/null &
cd ~/Ruty/apps || exit 1
sed -i 's/\r$//' run_sign4.sh run_sign4_tenants.sh
bash ~/Ruty/apps/run_sign4.sh "std turbo mini hybrid fw fw-turbo fw-mini fw-hybrid" 25 word > ~/Ruty/apps/run_sign4_modes.log 2>&1
bash ~/Ruty/apps/run_sign4_tenants.sh tenants 25 word > ~/Ruty/apps/run_sign4_tenants.log 2>&1
echo "ALL DONE $(date +%H:%M:%S)"
