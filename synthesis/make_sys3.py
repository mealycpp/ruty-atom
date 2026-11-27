# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""build_sys2.tcl -> build_sys3.tcl: N RUTY engines sharing one memory port (S_AXI_HP0_FPD) through one interconnect.
vivado -mode batch -source build_sys3.tcl -tclargs <M> <bus MHz> <engine MHz> <N engines>"""
import os; os.chdir(os.path.dirname(os.path.abspath(__file__)))
s = open("build_sys2.tcl").read()
def rep(a, b):
    global s
    assert s.count(a) == 1, a; s = s.replace(a, b)
rep("set emhz  [lindex $argv 2]", "set emhz  [lindex $argv 2]\nset N     [lindex $argv 3]")
rep("set name  ruty_v1_m${M}_${bmhz}_${emhz}", "set name  ruty_v4_n${N}_m${M}_${bmhz}_${emhz}")
rep("set_property -dict [list CONFIG.NUM_SI {1} CONFIG.NUM_MI {1}] $sc0", "set_property -dict [list CONFIG.NUM_SI {1} CONFIG.NUM_MI $N] $sc0")
rep("set_property -dict [list CONFIG.NUM_SI {1} CONFIG.NUM_MI {1}] $sc1", "set_property -dict [list CONFIG.NUM_SI $N CONFIG.NUM_MI {1}] $sc1")
rep("set core [create_bd_cell -type module -reference ruty_top ruty_0]\nset_property CONFIG.SLICES [expr {2 * $M}] $core\n",
    "for {set i 0} {$i < $N} {incr i} {\n    set core [create_bd_cell -type module -reference ruty_top ruty_$i]\n    set_property CONFIG.SLICES [expr {2 * $M}] $core\n}\n")
rep("    [get_bd_pins sc_reg/aclk] [get_bd_pins sc_mem/aclk] [get_bd_pins ruty_0/aclk] [get_bd_pins clk_wiz_0/clk_in1]\n",
    "    [get_bd_pins sc_reg/aclk] [get_bd_pins sc_mem/aclk] [get_bd_pins clk_wiz_0/clk_in1]\n")
rep("connect_bd_net [get_bd_pins clk_wiz_0/clk_out1] [get_bd_pins ruty_0/eclk]\n", "")
rep("connect_bd_net [get_bd_pins rst_pl0/peripheral_aresetn] [get_bd_pins sc_reg/aresetn] [get_bd_pins sc_mem/aresetn] [get_bd_pins ruty_0/aresetn]\n",
    "connect_bd_net [get_bd_pins rst_pl0/peripheral_aresetn] [get_bd_pins sc_reg/aresetn] [get_bd_pins sc_mem/aresetn]\n")
rep("connect_bd_intf_net [get_bd_intf_pins sc_reg/M00_AXI] [get_bd_intf_pins ruty_0/s_axi]\nconnect_bd_intf_net [get_bd_intf_pins ruty_0/m_axi] [get_bd_intf_pins sc_mem/S00_AXI]\n",
    "for {set i 0} {$i < $N} {incr i} {\n    connect_bd_net [get_bd_pins zynq_ultra_ps_e_0/pl_clk0] [get_bd_pins ruty_$i/aclk]\n    connect_bd_net [get_bd_pins clk_wiz_0/clk_out1] [get_bd_pins ruty_$i/eclk]\n    connect_bd_net [get_bd_pins rst_pl0/peripheral_aresetn] [get_bd_pins ruty_$i/aresetn]\n    connect_bd_intf_net [get_bd_intf_pins sc_reg/M0${i}_AXI] [get_bd_intf_pins ruty_$i/s_axi]\n    connect_bd_intf_net [get_bd_intf_pins ruty_$i/m_axi] [get_bd_intf_pins sc_mem/S0${i}_AXI]\n}\n")
rep("foreach seg [get_bd_addr_segs -of_objects [get_bd_addr_spaces zynq_ultra_ps_e_0/Data]] {\n    set_property range 4K $seg; set_property offset 0xA0000000 $seg }\n",
    "for {set i 0} {$i < $N} {incr i} {\n    foreach seg [get_bd_addr_segs -of_objects [get_bd_addr_spaces zynq_ultra_ps_e_0/Data]] {\n        if {[string match *ruty_${i}_* $seg]} { set_property range 4K $seg; set_property offset [format 0x%X [expr {0xA0000000 + 0x1000 * $i}]] $seg; puts \"RUTY_REG_SEG=$seg off=[get_property offset $seg]\" } } }\n")
open("build_sys3.tcl", "w").write(s)
print("ok")
