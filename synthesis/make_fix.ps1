# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Make the two fixed-width variants of the atom system for the like-for-like comparison (RQ1).
#   rtl_fix_w : dual tied to 0 (one Keccak-p[1600] only)      rtl_fix_n : dual tied to 1 (two Keccak-p[800] only)
# Same sequencer, same shell, same port. Only the register bit that selects the width is replaced by a constant,
# so synthesis removes the width muxes. Nothing in rtl_atom is changed.
$root = "C:\Users\elhad\ruty"
foreach ($k in @(@("w","1'b0"), @("n","1'b1"))) {
  $tag = $k[0]; $c = $k[1]; $d = "$root\rtl_fix_$tag"
  New-Item -ItemType Directory -Force -Path $d | Out-Null
  Copy-Item "$root\rtl_atom\atom_seq3.v" "$d\atom_seq3.v" -Force
  Copy-Item "$root\rtl_atom\ruty_fifo.v" "$d\ruty_fifo.v" -Force
  $t = Get-Content "$root\rtl_atom\ruty_top3.v" -Raw
  $n0 = ([regex]::Matches($t, "dual <= s_axi_wdata\[16\];")).Count
  $n1 = ([regex]::Matches($t, "njobs <= 24'd0; dual <= 1'b0;")).Count
  $t = $t.Replace("dual <= s_axi_wdata[16];", "dual <= $c;").Replace("njobs <= 24'd0; dual <= 1'b0;", "njobs <= 24'd0; dual <= $c;")
  Set-Content -Path "$d\ruty_top3.v" -Value $t -NoNewline -Encoding ascii
  "fix_$tag : write sites replaced=$n0 reset sites replaced=$n1 ; remaining 'dual <=' lines:"
  Select-String -Path "$d\ruty_top3.v" -Pattern "dual <= " | ForEach-Object { "   " + ($_.Matches.Count) + " match(es) on line " + $_.LineNumber }
  $b = Get-Content "$root\vivado\build_atom3.tcl" -Raw
  $b = $b.Replace('set name  atom_v3_n${N}_m${M}_${bmhz}_${emhz}', "set name  atom_fix${tag}_n`${N}_m`${M}_`${bmhz}_`${emhz}").Replace('$root/rtl_atom/', "`$root/rtl_fix_$tag/")
  Set-Content -Path "$root\vivado\build_fix_$tag.tcl" -Value $b -NoNewline -Encoding ascii
  Select-String -Path "$root\vivado\build_fix_$tag.tcl" -Pattern "^set name|add_files -norecurse \[list" | ForEach-Object { "   " + $_.Line }
}
