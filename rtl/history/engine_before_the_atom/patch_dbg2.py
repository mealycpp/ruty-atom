# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""v21: debug register 0x40 also shows the live write-channel pins and counts write beats and write responses."""
s = open("ruty_top.v").read()
def rep(a, b):
    global s; assert s.count(a) == 1, a[:40]; s = s.replace(a, b)
rep("8'd20}", "8'd21}")
rep("  assign dbg1 = {wtot[15:0], {(14-IAW){1'b0}}, outst};",
    "  reg [5:0] wcnt; reg [3:0] bcnt, awcnt;\n  always @(posedge aclk) if (go) begin wcnt <= 0; bcnt <= 0; awcnt <= 0; end else begin wcnt <= wcnt + wbeatgo; bcnt <= bcnt + (m_axi_bvalid & m_axi_bready); awcnt <= awcnt + (m_axi_awvalid & m_axi_awready); end\n"
    "  assign dbg1 = {wtot[7:0], m_axi_bvalid, m_axi_wready, m_axi_awready, m_axi_wlast, awcnt, bcnt, wcnt, wbeat[5:0]};")
open("ruty_top.v", "w").write(s)
