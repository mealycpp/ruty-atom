# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Run-time atom at higher engine clocks, for the RQ1 clock comparison.  usage: queue_rt.ps1 "480 500"
param([string]$jobs)
foreach ($e in $jobs.Split(' ')) {
  $log = "C:\Users\elhad\ruty\logs\v3_n1_m16_300_${e}.log"
  Add-Content "C:\Users\elhad\ruty\logs\queue_rt.txt" "$(Get-Date -Format HH:mm:ss) start runtime engine=$e"
  & cmd.exe /c "C:\Xilinx\Vivado\2022.2\bin\vivado.bat" -mode batch -nojournal -log $log -source C:\Users\elhad\ruty\vivado\build_atom3.tcl -tclargs 16 300 $e 1 | Out-Null
  Add-Content "C:\Users\elhad\ruty\logs\queue_rt.txt" "$(Get-Date -Format HH:mm:ss) end   runtime engine=$e"
}