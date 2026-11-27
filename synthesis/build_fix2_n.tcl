# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# RUTY bus-master image for the AUP-ZU3. Two clocks: bus at <bus MHz> from the processor, engine at <engine MHz> from a clock generator.
#   processor --(fast port, AXI4-Lite)--> RUTY registers
#   RUTY --(AXI4 master, 64-bit)--> S_AXI_HP0_FPD --> DDR
# vivado -mode batch -source build_sys2.tcl -tclargs <slice units M> <bus MHz> <engine MHz>
set M     [lindex $argv 0]
set bmhz  [lindex $argv 1]
set emhz  [lindex $argv 2]
set N     [lindex $argv 3]
set root  C:/Users/elhad/ruty
set name  atom_fix2n_n${N}_m${M}_${bmhz}_${emhz}
set proj  $root/vivado/sys/$name
set part  xczu3eg-sfvc784-2-e
create_project $name $proj -part $part -force
set_property target_language Verilog [current_project]
add_files -norecurse [list $root/rtl_fix2_n/atom_seq3.v $root/rtl_fix2_n/ruty_fifo.v $root/rtl_fix2_n/ruty_top3.v]
update_compile_order -fileset sources_1
create_bd_design ruty
set ps [create_bd_cell -type ip -vlnv xilinx.com:ip:zynq_ultra_ps_e:3.4 zynq_ultra_ps_e_0]
set_property -dict [list \
    CONFIG.PSU__USE__M_AXI_GP0 {1} CONFIG.PSU__USE__M_AXI_GP1 {0} CONFIG.PSU__USE__M_AXI_GP2 {0} \
    CONFIG.PSU__MAXIGP0__DATA_WIDTH {32} \
    CONFIG.PSU__USE__S_AXI_GP2 {1} CONFIG.PSU__SAXIGP2__DATA_WIDTH {64} \
    CONFIG.PSU__CRL_APB__PL0_REF_CTRL__SRCSEL {IOPLL} \
    CONFIG.PSU__CRL_APB__PL0_REF_CTRL__FREQMHZ $bmhz ] $ps
set rst [create_bd_cell -type ip -vlnv xilinx.com:ip:proc_sys_reset:5.0 rst_pl0]
set cw  [create_bd_cell -type ip -vlnv xilinx.com:ip:clk_wiz:6.0 clk_wiz_0]
set fin [expr {[get_property CONFIG.FREQ_HZ [get_bd_pins zynq_ultra_ps_e_0/pl_clk0]] / 1000000.0}]
puts "RUTY_CLKIN=$fin"
set_property -dict [list CONFIG.PRIM_IN_FREQ $fin CONFIG.CLKOUT1_REQUESTED_OUT_FREQ $emhz CONFIG.USE_LOCKED {true} CONFIG.USE_RESET {false} \
    CONFIG.PRIM_SOURCE {Global_buffer}] $cw
set sc0 [create_bd_cell -type ip -vlnv xilinx.com:ip:smartconnect:1.0 sc_reg]
set_property -dict [list CONFIG.NUM_SI {1} CONFIG.NUM_MI $N] $sc0
set sc1 [create_bd_cell -type ip -vlnv xilinx.com:ip:smartconnect:1.0 sc_mem]
set_property -dict [list CONFIG.NUM_SI $N CONFIG.NUM_MI {1}] $sc1
for {set i 0} {$i < $N} {incr i} {
    set core [create_bd_cell -type module -reference ruty_top ruty_$i]
    set_property CONFIG.SLICES [expr {2 * $M}] $core
}
connect_bd_net [get_bd_pins zynq_ultra_ps_e_0/pl_clk0] \
    [get_bd_pins zynq_ultra_ps_e_0/maxihpm0_fpd_aclk] [get_bd_pins zynq_ultra_ps_e_0/saxihp0_fpd_aclk] [get_bd_pins rst_pl0/slowest_sync_clk] \
    [get_bd_pins sc_reg/aclk] [get_bd_pins sc_mem/aclk] [get_bd_pins clk_wiz_0/clk_in1]
connect_bd_net [get_bd_pins clk_wiz_0/locked] [get_bd_pins rst_pl0/dcm_locked]
connect_bd_net [get_bd_pins zynq_ultra_ps_e_0/pl_resetn0] [get_bd_pins rst_pl0/ext_reset_in]
connect_bd_net [get_bd_pins rst_pl0/peripheral_aresetn] [get_bd_pins sc_reg/aresetn] [get_bd_pins sc_mem/aresetn]
connect_bd_intf_net [get_bd_intf_pins zynq_ultra_ps_e_0/M_AXI_HPM0_FPD] [get_bd_intf_pins sc_reg/S00_AXI]
for {set i 0} {$i < $N} {incr i} {
    connect_bd_net [get_bd_pins zynq_ultra_ps_e_0/pl_clk0] [get_bd_pins ruty_$i/aclk]
    connect_bd_net [get_bd_pins clk_wiz_0/clk_out1] [get_bd_pins ruty_$i/eclk]
    connect_bd_net [get_bd_pins rst_pl0/peripheral_aresetn] [get_bd_pins ruty_$i/aresetn]
    connect_bd_intf_net [get_bd_intf_pins sc_reg/M0${i}_AXI] [get_bd_intf_pins ruty_$i/s_axi]
    connect_bd_intf_net [get_bd_intf_pins ruty_$i/m_axi] [get_bd_intf_pins sc_mem/S0${i}_AXI]
}
connect_bd_intf_net [get_bd_intf_pins sc_mem/M00_AXI] [get_bd_intf_pins zynq_ultra_ps_e_0/S_AXI_HP0_FPD]
assign_bd_address
for {set i 0} {$i < $N} {incr i} {
    foreach seg [get_bd_addr_segs -of_objects [get_bd_addr_spaces zynq_ultra_ps_e_0/Data]] {
        if {[string match *ruty_${i}_* $seg]} { set_property range 4K $seg; set_property offset [format 0x%X [expr {0xA0000000 + 0x1000 * $i}]] $seg; puts "RUTY_REG_SEG=$seg off=[get_property offset $seg]" } } }
foreach seg [get_bd_addr_segs -of_objects [get_bd_addr_spaces ruty_0/m_axi]] {
    puts "RUTY_MEM_SEG=$seg off=[get_property offset $seg] range=[get_property range $seg]" }
validate_bd_design
save_bd_design
make_wrapper -files [get_files $proj/$name.srcs/sources_1/bd/ruty/ruty.bd] -top
add_files -norecurse $proj/$name.gen/sources_1/bd/ruty/hdl/ruty_wrapper.v
set_property top ruty_wrapper [current_fileset]
# the two clocks are joined only through the FIFOs and synchronisers: treat them as unrelated
set xdc $proj/ruty_cdc.xdc
set f [open $xdc w]
puts $f {set_clock_groups -asynchronous -group [get_clocks clk_pl_0] -group [get_clocks -of_objects [get_pins ruty_i/clk_wiz_0/inst/mmcme4_adv_inst/CLKOUT0]]}
close $f
add_files -fileset constrs_1 -norecurse $xdc
set_property used_in_synthesis false [get_files $xdc]
update_compile_order -fileset sources_1
set_property STRATEGY Performance_ExplorePostRoutePhysOpt [get_runs impl_1]
launch_runs synth_1 -jobs 8
wait_on_run synth_1
if {[get_property PROGRESS [get_runs synth_1]] != "100%"} { puts "RUTY_SYNTH_FAILED"; exit 1 }
launch_runs impl_1 -to_step write_bitstream -jobs 8
wait_on_run impl_1
open_run impl_1
report_timing_summary -max_paths 10 -file $proj/route_timing.rpt
report_utilization -hierarchical -hierarchical_depth 5 -file $proj/route_util_hier.rpt
report_clocks -file $proj/clocks.rpt
report_cdc -file $proj/cdc.rpt
set line "RUTY_IMAGE $name"
foreach c [get_clocks] {
    set p [get_timing_paths -setup -max_paths 1 -group $c]
    if {[llength $p]} { append line " [get_property NAME $c]:period=[get_property PERIOD $c],wns=[get_property SLACK $p]" } }
append line " whs=[get_property SLACK [get_timing_paths -delay_type min]]"
set bit [lindex [glob -nocomplain $proj/$name.runs/impl_1/*.bit] 0]
set hwh [lindex [glob -nocomplain $proj/$name.gen/sources_1/bd/ruty/hw_handoff/*.hwh] 0]
file mkdir $root/images/$name
if {$bit ne ""} { file copy -force $bit $root/images/$name/ruty.bit }
if {$hwh ne ""} { file copy -force $hwh $root/images/$name/ruty.hwh }
append line " bit=[expr {$bit ne {}}]"
puts $line
set f [open $root/vivado/results_sys.txt a]; puts $f $line; close $f
