# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Run a list of system builds one after another.  usage: queue.ps1 <lane name> "<engineMHz>:<engines> <engineMHz>:<engines> ..."
param([string]$lane, [string]$jobs)
foreach ($j in $jobs.Split(' ')) {
  $e, $n = $j.Split(':')
  $log = "C:\Users\elhad\ruty\logs\sys8_n${n}_m16_300_${e}.log"
  Add-Content "C:\Users\elhad\ruty\logs\queue_$lane.txt" "$(Get-Date -Format HH:mm:ss) start n=$n engine=$e"
  & cmd.exe /c "C:\Xilinx\Vivado\2022.2\bin\vivado.bat" -mode batch -nojournal -log $log -source C:\Users\elhad\ruty\vivado\build_sys3.tcl -tclargs 16 300 $e $n | Out-Null
  Add-Content "C:\Users\elhad\ruty\logs\queue_$lane.txt" "$(Get-Date -Format HH:mm:ss) end   n=$n engine=$e"
}
