# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""v22: write data is offered only after the write address has been accepted, so the address register never moves under a pending request."""
s = open("ruty_top.v").read()
def rep(a, b):
    global s; assert s.count(a) == 1, a[:40]; s = s.replace(a, b)
rep("8'd21}", "8'd22}")
rep("assign m_axi_wvalid = (ws == M_DATA) & (ovalid | werr);", "assign m_axi_wvalid = (ws == M_DATA) & ~m_axi_awvalid & (ovalid | werr);")
open("ruty_top.v", "w").write(s)
