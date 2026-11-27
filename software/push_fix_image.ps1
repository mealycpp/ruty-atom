# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# copy one image (ruty.bit + ruty.hwh) to ~/Ruty/<name>/ on aup1 and print both md5 sums
param([string]$name)
$src = "C:\Users\elhad\ruty\images\$name"
if (-not (Test-Path "$src\ruty.bit") -or -not (Test-Path "$src\ruty.hwh")) { "NO IMAGE $name"; exit 1 }
ssh -o BatchMode=yes xilinx@aup1 "mkdir -p Ruty/$name"
scp -o BatchMode=yes "$src\ruty.bit" "$src\ruty.hwh" "xilinx@aup1:Ruty/$name/"
"pc    " + (Get-FileHash "$src\ruty.bit" -Algorithm MD5).Hash.ToLower()
ssh -o BatchMode=yes xilinx@aup1 "md5sum Ruty/$name/ruty.bit; ls -l Ruty/$name"
