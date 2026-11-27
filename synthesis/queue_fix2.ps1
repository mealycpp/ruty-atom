# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Fixed-width comparison builds, one after another.  usage: queue_fix.ps1 "<w|n>:<engineMHz> ..."   (one atom, bus 300 MHz)
param([string]$jobs)
foreach ($j in $jobs.Split(' ')) {
  $k, $e = $j.Split(':')
  $log = "C:\Users\elhad\ruty\logs\fix2${k}_n1_m16_300_${e}.log"
  Add-Content "C:\Users\elhad\ruty\logs\queue_fix2.txt" "$(Get-Date -Format HH:mm:ss) start kind=$k engine=$e"
  & cmd.exe /c "C:\Xilinx\Vivado\2022.2\bin\vivado.bat" -mode batch -nojournal -log $log -source C:\Users\elhad\ruty\vivado\build_fix2_$k.tcl -tclargs 16 300 $e 1 | Out-Null
  Add-Content "C:\Users\elhad\ruty\logs\queue_fix2.txt" "$(Get-Date -Format HH:mm:ss) end   kind=$k engine=$e"
}
