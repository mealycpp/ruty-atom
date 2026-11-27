# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""ruty_top.v: the two batch-size products get a three-stage pipeline (they were one long path through the multiplier)."""
import os; os.chdir(os.path.dirname(os.path.abspath(__file__)))
s = open("ruty_top.v").read()
def rep(a, b):
    global s
    assert s.count(a) == 1, a; s = s.replace(a, b)
rep("  reg [44:0] rtot, wtot; reg [1:0] rs, ws; reg [7:0] wbeat; reg mulv;",
    "  reg [44:0] rtot, wtot; reg [1:0] rs, ws; reg [7:0] wbeat; reg mulv; reg [3:0] gd;\n  reg [23:0] nj_r; reg [20:0] wpm_r; reg [13:0] wpo_r; reg [44:0] pr1, pr2, pw1, pw2;       // product pipeline\n  always @(posedge aclk) begin nj_r <= njobs; wpm_r <= wpm; wpo_r <= wpo; pr1 <= nj_r * wpm_r; pr2 <= pr1; pw1 <= nj_r * wpo_r; pw2 <= pw1; end")
rep("mulv <= 1'b0; clocks <= 32'd0; end", "mulv <= 1'b0; gd <= 4'd0; clocks <= 32'd0; end")
rep("      mulv <= go;\n", "      gd <= {gd[2:0], go}; mulv <= gd[3];                                                      // wait for the products\n")
rep("rtot <= njobs * wpm; wtot <= njobs * wpo; m_axi_araddr <= in_a;", "m_axi_araddr <= in_a;")
rep("      if (mulv) begin go_t <= ~go_t;", "      if (mulv) begin rtot <= pr2; wtot <= pw2; go_t <= ~go_t;")
rep("if (busy & ~mulv & ~go & (rs == M_IDLE)", "if (busy & ~mulv & ~go & (gd == 4'd0) & (rs == M_IDLE)")
rep("8'd16};", "8'd17};")
open("ruty_top.v", "w").write(s)
