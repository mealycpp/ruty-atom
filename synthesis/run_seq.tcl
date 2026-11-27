# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# Out-of-context synthesis, place and route of one engine at one clock target.
# vivado -mode batch -source run.tcl -tclargs <file stem> <module> <period ns> [directive]
set stem   [lindex $argv 0]
set mod    [lindex $argv 1]
set period [lindex $argv 2]
set dir    [expr {[llength $argv] > 3 ? [lindex $argv 3] : "Default"}]
set part   xczu3eg-sfvc784-2-e
set root   C:/Users/elhad/ruty
set out    $root/vivado/out/${stem}_${period}_bufg
file mkdir $out
set_param general.maxThreads 4
read_verilog $root/rtl_atom/$stem.v
read_verilog $root/vivado/wrap_seq.v
synth_design -top wrap -part $part -mode out_of_context -verilog_define CORE=$mod -flatten_hierarchy rebuilt
create_clock -name clk -period $period [get_ports clk]
set_property HD.CLK_SRC BUFGCE_X0Y0 [get_ports clk]
report_utilization -hierarchical -file $out/util_synth.rpt
opt_design
if {$dir eq "Default"} { place_design } else { place_design -directive $dir }
phys_opt_design
route_design
phys_opt_design
report_timing_summary -file $out/timing.rpt
report_utilization -hierarchical -file $out/util.rpt
report_utilization -file $out/util_flat.rpt
report_timing -max_paths 3 -file $out/paths.rpt
report_design_analysis -logic_level_distribution -file $out/levels.rpt
set wns [get_property SLACK [get_timing_paths -max_paths 1 -setup]]
set whs [get_property SLACK [get_timing_paths -max_paths 1 -hold]]
set lvl [get_property LOGIC_LEVELS [get_timing_paths -max_paths 1 -setup]]
set lut [llength [get_cells -hier -filter {PRIMITIVE_GROUP == CLB && REF_NAME =~ LUT* && NAME =~ core/*}]]
set ff  [llength [get_cells -hier -filter {PRIMITIVE_GROUP == REGISTER && NAME =~ core/*}]]
set mux [llength [get_cells -hier -filter {REF_NAME =~ MUXF* && NAME =~ core/*}]]
set fmax [expr {1000.0 / ($period - $wns)}]
set line [format "RESULT bufg %s period=%.3f wns=%.3f whs=%.3f fmax=%.1f levels=%s lut=%d ff=%d muxf=%d" $stem $period $wns $whs $fmax $lvl $lut $ff $mux]
puts $line
set f [open $root/vivado/results.txt a]; puts $f $line; close $f
