# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
set_clock_groups -asynchronous -group [get_clocks clk_pl_0] -group [get_clocks -of_objects [get_pins ruty_i/clk_wiz_0/inst/mmcme4_adv_inst/CLKOUT0]]
