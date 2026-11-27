#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# start run_fixcheck.sh detached, only if no ruty_ program is running
cd ~/Ruty/apps || exit 1
if pgrep -a ruty_ ; then echo "BUSY: a ruty_ program is running, not started"; exit 1; fi
sed -i 's/\r$//' run_fixcheck.sh ruty_fixcheck.rs fixcheck_specs.txt
nohup bash run_fixcheck.sh > run_fixcheck.log 2>&1 < /dev/null &
echo "STARTED"
