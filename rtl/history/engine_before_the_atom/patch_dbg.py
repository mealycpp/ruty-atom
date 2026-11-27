# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""v20: two read-only registers that show where the bus master is (0x3C, 0x40)."""
s = open("ruty_top.v").read()
def rep(a, b):
    global s; assert s.count(a) == 1, a[:40]; s = s.replace(a, b)
rep("8'd19}", "8'd20}")
rep("          default: s_axi_rdata <= 32'd0;", "          6'd15: s_axi_rdata <= dbg0; 6'd16: s_axi_rdata <= dbg1;\n          default: s_axi_rdata <= 32'd0;")
rep("  // ---------------- registers (aclk) ----------------", "  wire [31:0] dbg0, dbg1;\n  // ---------------- registers (aclk) ----------------")
rep("  wire arhs = m_axi_arvalid & m_axi_arready;", "  wire arhs = m_axi_arvalid & m_axi_arready;\n  assign dbg0 = {ws, rs, si2, ovalid, ifull, ocount, mulv, go_t, m_axi_awvalid, m_axi_arvalid, wl, 4'd0};\n  assign dbg1 = {wtot[15:0], {(14-IAW){1'b0}}, outst};")
open("ruty_top.v", "w").write(s)
