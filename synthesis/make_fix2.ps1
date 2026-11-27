# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Fixed-width variants, second method: the width input of the sequencer and the two width-dependent expressions in the top
# are tied to a LITERAL constant, so synthesis is certain to remove the width muxes. (First method only made the register constant;
# its build came out the same size as the run-time atom, which may mean the logic was not trimmed.)  rtl_fix2_w = 1x1600, rtl_fix2_n = 2x800
$root = "C:\Users\elhad\ruty"
foreach ($k in @(@("w","1'b0"), @("n","1'b1"))) {
  $tag = $k[0]; $c = $k[1]; $d = "$root\rtl_fix2_$tag"
  New-Item -ItemType Directory -Force -Path $d | Out-Null
  Copy-Item "$root\rtl_atom\atom_seq3.v" "$d\atom_seq3.v" -Force; Copy-Item "$root\rtl_atom\ruty_fifo.v" "$d\ruty_fifo.v" -Force
  $t = Get-Content "$root\rtl_atom\ruty_top3.v" -Raw
  $pairs = @(@(".dual(dual)", ".dual($c)"), @("wire [20:0] wpm = dual ?", "wire [20:0] wpm = $c ?"), @("wire [13:0] wpo = dual ?", "wire [13:0] wpo = $c ?"))
  foreach ($p in $pairs) { $n = ([regex]::Matches($t, [regex]::Escape($p[0]))).Count; "fix2_$tag : '$($p[0])' found $n"; $t = $t.Replace($p[0], $p[1]) }
  [IO.File]::WriteAllText("$d\ruty_top3.v", $t)
  "   other uses of dual left in the top: " + (Select-String -Path "$d\ruty_top3.v" -Pattern "\bdual\b" -AllMatches | ForEach-Object { $_.Matches.Count } | Measure-Object -Sum).Sum
  $b = (Get-Content "$root\vivado\build_atom3.tcl" -Raw).Replace('set name  atom_v3_n${N}_m${M}_${bmhz}_${emhz}', "set name  atom_fix2${tag}_n`${N}_m`${M}_`${bmhz}_`${emhz}").Replace('$root/rtl_atom/', "`$root/rtl_fix2_$tag/")
  [IO.File]::WriteAllText("$root\vivado\build_fix2_$tag.tcl", $b)
}
$q = (Get-Content "$root\vivado\queue_fix.ps1" -Raw).Replace("build_fix_", "build_fix2_").Replace("logs\fix", "logs\fix2").Replace("queue_fix.txt", "queue_fix2.txt")
[IO.File]::WriteAllText("$root\vivado\queue_fix2.ps1", $q)
