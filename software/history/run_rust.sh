#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board: build the stand-alone Rust driver, load an image, run.   bash run_rust.sh <dir> <engines>
cd ~/Ruty/$1 || exit 1
N=$2
md5sum ruty.rs cacheops.S
gcc -c cacheops.S -o cacheops.o && rustc -O -C link-arg=cacheops.o -o ruty ruty.rs 2>&1 | grep -E "^error" -A5 | head -20
./ruty selftest | head -2 | cut -c1-80
sudo -n bash -c "grep CmaFree /proc/meminfo"
sudo -n bash -c 'source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 -c "from pynq import Overlay; Overlay(\"ruty.bit\"); print(\"loaded\")"' 2>&1 | tail -1
sudo -n ./ruty $N 300 2>&1 | tee rust_n$N.log
