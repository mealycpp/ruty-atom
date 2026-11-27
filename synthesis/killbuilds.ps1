# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Stop every RUTY build on this PC (queue lanes and their Vivado processes). Touches nothing outside C:\Users\elhad\ruty.
$all = Get-CimInstance Win32_Process
$ids = @(($all | Where-Object { $_.CommandLine -match 'ruty\\vivado\\queue\.ps1|ruty\\vivado\\build_sys|ruty/vivado/sys/' -and $_.CommandLine -notmatch 'killbuilds' }).ProcessId)
for ($i = 0; $i -lt 6; $i++) { $ids += @(($all | Where-Object { $ids -contains $_.ParentProcessId }).ProcessId) }
$ids = $ids | Where-Object { $_ -and $_ -ne $PID } | Sort-Object -Unique
"stopping $($ids.Count) processes"
$ids | ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }
