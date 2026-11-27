#!/bin/bash
# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
cd /mnt/c/Users/elhad/ruty/rtl
python3 gen.py 8; python3 gen.py 8 single; python3 gen.py 8 dual
python3 gen.py 16; python3 gen.py 16 single; python3 gen.py 16 dual
python3 gen.py 4; python3 gen.py 32
md5sum *.v
