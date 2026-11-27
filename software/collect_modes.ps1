# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# collect_modes.ps1 (NEW): copy the ruty_modes / modes_bench / xkcp_modes_bench logs from aup1 into results\ under new modes_* names, then run the Python check.
$r = "C:\Users\elhad\ruty\results"
$map = @{ "modes_kt_atom_v3_n1_450.log" = "modes_kt_1atoms_aup1.txt"; "modes_kt_atom_v3_n2_420.log" = "modes_kt_2atoms_aup1.txt"; "modes_kt_atom_v3_n4_400.log" = "modes_kt_4atoms_aup1.txt";
  "modes_merkle_atom_v3_n1_450.log" = "modes_merkle_1atoms_aup1.txt"; "modes_merkle_atom_v3_n2_420.log" = "modes_merkle_2atoms_aup1.txt"; "modes_merkle_atom_v3_n4_400.log" = "modes_merkle_4atoms_aup1.txt";
  "modes_sw.log" = "modes_sw_a53_aup1.txt"; "modes_xkcp_digests.log" = "modes_xkcp_digests_aup1.txt"; "modes_xkcp_roots.log" = "modes_xkcp_roots_aup1.txt"; "modes_xkcp_sw.log" = "modes_xkcp_sw_a53_aup1.txt"; "run_modes.log" = "modes_run_log_aup1.txt" }
foreach ($k in $map.Keys) { $dst = Join-Path $r $map[$k]; if (Test-Path $dst) { "EXISTS, not overwritten: $dst"; continue }; scp -o BatchMode=yes "xilinx@aup1:Ruty/apps/$k" $dst 2>&1 | Out-Null; if (Test-Path $dst) { "copied $k -> $($map[$k])" } else { "missing $k" } }
$files = @("modes_kt_1atoms_aup1.txt","modes_kt_2atoms_aup1.txt","modes_kt_4atoms_aup1.txt","modes_merkle_1atoms_aup1.txt","modes_merkle_2atoms_aup1.txt","modes_merkle_4atoms_aup1.txt") | ForEach-Object { Join-Path $r $_ } | Where-Object { Test-Path $_ }
$chk = Join-Path $r "modes_check_python.txt"
if (-not (Test-Path $chk)) { python C:\Users\elhad\ruty\board\check_modes.py C:\Users\elhad\ruty\pyref_modes @files > $chk 2>&1; Get-Content $chk | Select-Object -Last 4 }
