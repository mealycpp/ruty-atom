# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Launch one Vivado run in the background. usage: .\go.ps1 <file stem> <module> <period ns> [directive]
param([string]$stem, [string]$mod, [string]$period, [string]$dir = "Default")
$root = "C:\Users\elhad\ruty"
$work = "$root\vivado\work\${stem}_${period}_bufg"
New-Item -ItemType Directory -Force $work | Out-Null
Start-Process -FilePath "C:\Xilinx\Vivado\2022.2\bin\vivado.bat" -WorkingDirectory $work -WindowStyle Hidden `
  -ArgumentList "-mode batch -nojournal -log $root\logs\${stem}_${period}_bufg.log -source $root\vivado\run.tcl -tclargs $stem $mod $period $dir"
