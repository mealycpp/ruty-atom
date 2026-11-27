# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# RUTY bring-up image for the AUP-ZU3: processor <-> AXI4-Lite registers <-> RUTY engine, one clock.
# vivado -mode batch -source build_sys.tcl -tclargs <slice units M> <clock MHz>
set M     [lindex $argv 0]
set mhz   [lindex $argv 1]
set root  C:/Users/elhad/ruty
set name  ruty_v0_m${M}_${mhz}
set proj  $root/vivado/sys/$name
set part  xczu3eg-sfvc784-2-e
create_project $name $proj -part $part -force
set_property target_language Verilog [current_project]
add_files -norecurse [list $root/rtl/ruty_core_m${M}.v $root/rtl/ruty_axil.v]
update_compile_order -fileset sources_1
create_bd_design ruty
set ps [create_bd_cell -type ip -vlnv xilinx.com:ip:zynq_ultra_ps_e:3.4 zynq_ultra_ps_e_0]
set_property -dict [list \
    CONFIG.PSU__USE__M_AXI_GP0 {1} CONFIG.PSU__USE__M_AXI_GP1 {0} CONFIG.PSU__USE__M_AXI_GP2 {0} \
    CONFIG.PSU__MAXIGP0__DATA_WIDTH {32} \
    CONFIG.PSU__CRL_APB__PL0_REF_CTRL__SRCSEL {IOPLL} \
    CONFIG.PSU__CRL_APB__PL0_REF_CTRL__FREQMHZ $mhz ] $ps
set rst [create_bd_cell -type ip -vlnv xilinx.com:ip:proc_sys_reset:5.0 rst_pl0]
set sc  [create_bd_cell -type ip -vlnv xilinx.com:ip:smartconnect:1.0 axi_sc]
set_property -dict [list CONFIG.NUM_SI {1} CONFIG.NUM_MI {1}] $sc
set core [create_bd_cell -type module -reference ruty_axil ruty_0]
set_property CONFIG.SLICES [expr {2 * $M}] $core
connect_bd_net [get_bd_pins zynq_ultra_ps_e_0/pl_clk0] \
    [get_bd_pins zynq_ultra_ps_e_0/maxihpm0_fpd_aclk] [get_bd_pins rst_pl0/slowest_sync_clk] \
    [get_bd_pins axi_sc/aclk] [get_bd_pins ruty_0/aclk]
connect_bd_net [get_bd_pins zynq_ultra_ps_e_0/pl_resetn0] [get_bd_pins rst_pl0/ext_reset_in]
connect_bd_net [get_bd_pins rst_pl0/peripheral_aresetn] [get_bd_pins axi_sc/aresetn] [get_bd_pins ruty_0/aresetn]
connect_bd_intf_net [get_bd_intf_pins zynq_ultra_ps_e_0/M_AXI_HPM0_FPD] [get_bd_intf_pins axi_sc/S00_AXI]
connect_bd_intf_net [get_bd_intf_pins axi_sc/M00_AXI] [get_bd_intf_pins ruty_0/s_axi]
assign_bd_address
foreach seg [get_bd_addr_segs -of_objects [get_bd_addr_spaces zynq_ultra_ps_e_0/Data]] {
    set_property range 4K $seg; set_property offset 0xA0000000 $seg
    puts "RUTY_ADDR_SEG=$seg off=[get_property offset $seg] range=[get_property range $seg]" }
validate_bd_design
save_bd_design
make_wrapper -files [get_files $proj/$name.srcs/sources_1/bd/ruty/ruty.bd] -top
add_files -norecurse $proj/$name.gen/sources_1/bd/ruty/hdl/ruty_wrapper.v
set_property top ruty_wrapper [current_fileset]
update_compile_order -fileset sources_1
set_property STRATEGY Performance_ExplorePostRoutePhysOpt [get_runs impl_1]
launch_runs synth_1 -jobs 8
wait_on_run synth_1
if {[get_property PROGRESS [get_runs synth_1]] != "100%"} { puts "RUTY_SYNTH_FAILED"; exit 1 }
launch_runs impl_1 -to_step write_bitstream -jobs 8
wait_on_run impl_1
open_run impl_1
report_timing_summary -max_paths 10 -file $proj/route_timing.rpt
report_utilization -hierarchical -hierarchical_depth 4 -file $proj/route_util_hier.rpt
set wns [get_property SLACK [get_timing_paths -delay_type max]]
set whs [get_property SLACK [get_timing_paths -delay_type min]]
set bit [lindex [glob -nocomplain $proj/$name.runs/impl_1/*.bit] 0]
set hwh [lindex [glob -nocomplain $proj/$name.gen/sources_1/bd/ruty/hw_handoff/*.hwh] 0]
file mkdir $root/images/$name
if {$bit ne ""} { file copy -force $bit $root/images/$name/ruty.bit }
if {$hwh ne ""} { file copy -force $hwh $root/images/$name/ruty.hwh }
set line "RUTY_IMAGE $name clk=$mhz wns=$wns whs=$whs bit=$bit"
puts $line
set f [open $root/vivado/results_sys.txt a]; puts $f $line; close $f
