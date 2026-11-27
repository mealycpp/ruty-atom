#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board: run the NIST CAVP and RFC 9861 known-answer tests through the engine.  usage: run_kat.sh <dir under ~/Ruty>
cd ~/Ruty/$1 || exit 1
md5sum ruty_kat.py kat/*.rsp kat/rfc9861.txt | cut -c1-70
sudo -n bash -c 'source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; timeout 3000 python3 ruty_kat.py ruty.bit kat' 2>&1 | grep -E "RESULT|Error|error|Traceback|line " | tee kat.log
