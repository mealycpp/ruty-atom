#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board: build the Rust scheduler, load the image, run.   usage: run_sched.sh <dir under ~/Ruty> <engines> <engine MHz>
cd ~/Ruty/$1 || exit 1
gcc -c cacheops.S -o cacheops.o && rustc -O -C link-arg=cacheops.o -o ruty_sched ruty_sched.rs 2>&1 | grep -E "^error" -A12 | head -60
[ -x ruty_sched ] || exit 1
sudo -n bash -c 'source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c "from pynq import Overlay; Overlay(\"ruty.bit\"); print(\"loaded\")"' 2>&1 | tail -1
sudo -n timeout 300 ./ruty_sched $2 $3 2>&1 | tee sched.log
