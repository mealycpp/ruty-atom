#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# On the board: build the Rust driver, extract the reference model, load the image and run the kernels.
cd ~/Ruty/ruty_v1_600 || exit 1
md5sum ruty_drv.rs ruty_pqc.py
python3 - <<'EOF'
s = open("ruty_batch.py").read(); a = s.index("RHO = [["); b = s.index("bit = sys.argv[1]")
open("ruty_ref.py", "w").write("import hashlib\n" + s[a:b])
EOF
md5sum ruty_ref.py
rustc -O --crate-type cdylib -o libruty_drv.so ruty_drv.rs 2>&1 | grep -E "^error" | head -5
ls -la libruty_drv.so | cut -c1-70
sudo -n bash -c 'source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; timeout 500 python3 ruty_pqc.py ruty.bit 300' 2>&1 | grep -E "RESULT|Error|error|Traceback" | tee pqc.log
