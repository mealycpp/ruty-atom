# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""gen.py v2 -> v3: job control hardened after the independent review.
illegal start (request already waiting, bad round range) is ignored and flagged; lane writes and clears to a busy core are
ignored and flagged; the mode is taken only when both cores are idle; the spare state code falls to idle; clear strobe added."""
import os; os.chdir(os.path.dirname(os.path.abspath(__file__)))
s = open("gen_v2.py").read()
def rep(a, b, n=1):
    global s
    assert s.count(a) == n, (a, s.count(a)); s = s.replace(a, b)
rep("    input [4:0] ridx, output reg [63:0] rdata,", "    input [1:0] clr,                                                     // zero the A / B halves of every lane\n    input [4:0] ridx, output reg [63:0] rdata,")
rep("    output [1:0] busy, output [1:0] qfull);", "    output [1:0] busy, output [1:0] qfull, output reg [1:0] err);        // err: sticky, an illegal request was ignored\n  // rules, enforced here: no start while qfull or with first > last or last > 23; no lane write or clear to a busy core;\n  // dual is sampled only while both cores are idle.")
rep('D = {"": "dual", "single": "1\'b0", "dual": "1\'b1"}[FIX]', 'D = {"": "du_r", "single": "1\'b0", "dual": "1\'b1"}[FIX]')
rep("  wire du = {D};", "  reg du_r; reg [1:0] cl;\n  wire du = {D};\n  wire [1:0] bad = {{(first1 > last1) | (last1 > 5'd23), (first0 > last0) | (last0 > 5'd23)}};\n  wire [1:0] acc = start & ~pend & ~bad & {{du, 1'b1}};                    // accepted start pulses\n  wire [1:0] bz = {{du ? busy[1] : busy[0], busy[0]}};                     // the core that owns each half is busy")
rep("        default: if (pend[0]) begin n_st0 = PRE; n_th0 = 1'b1; take[0] = 1'b1; end", "        IDLE: if (pend[0]) begin n_st0 = PRE; n_th0 = 1'b1; take[0] = 1'b1; end\n        default: begin n_st0 = IDLE; n_th0 = 1'b0; end")
rep("        default: if (pend[1]) begin n_st1 = PRE; n_th1 = 1'b1; take[1] = 1'b1; end", "        IDLE: if (pend[1]) begin n_st1 = PRE; n_th1 = 1'b1; take[1] = 1'b1; end\n        default: begin n_st1 = IDLE; n_th1 = 1'b0; end")
rep("    wsA <= wr[0] ? (25'd1 << widx) : 25'd0; wsB <= wr[1] ? (25'd1 << widx) : 25'd0; wx <= wxor; wd <= wdata;\n    if (rst) begin st0 <= IDLE;",
    "    wx <= wxor; wd <= wdata;\n    if (rst) begin wsA <= 25'd0; wsB <= 25'd0; cl <= 2'b00; err <= 2'b00; du_r <= dual; st0 <= IDLE;")
rep("    else begin\n      cnt <= n_cnt;", "    else begin\n      wsA <= (wr[0] & ~bz[0]) ? (25'd1 << widx) : 25'd0; wsB <= (wr[1] & ~bz[1]) ? (25'd1 << widx) : 25'd0; cl <= clr & ~bz;\n      err <= err | (start & {{du, 1'b1}} & ~acc) | ((wr | clr) & bz);\n      if (busy == 2'b00 && start == 2'b00) du_r <= dual;\n      cnt <= n_cnt;")
rep("      if (take[0]) begin pend[0] <= start[0]; pf0 <= qf0; pl0 <= ql0; if (start[0]) begin qf0 <= first0; ql0 <= last0; end end\n      else if (start[0]) begin pend[0] <= 1'b1; qf0 <= first0; ql0 <= last0; end",
    "      if (take[0]) begin pend[0] <= 1'b0; pf0 <= qf0; pl0 <= ql0; end\n      else if (acc[0]) begin pend[0] <= 1'b1; qf0 <= first0; ql0 <= last0; end")
rep("      if (take[1]) begin pend[1] <= start[1] & du; pf1 <= qf1; pl1 <= ql1; if (start[1]) begin qf1 <= first1; ql1 <= last1; end end\n      else if (start[1] & du) begin pend[1] <= 1'b1; qf1 <= first1; ql1 <= last1; end",
    "      if (take[1]) begin pend[1] <= 1'b0; pf1 <= qf1; pl1 <= ql1; end\n      else if (acc[1]) begin pend[1] <= 1'b1; qf1 <= first1; ql1 <= last1; end")
rep("pasA <= (n_cnt != 0) & (nsA != IDLE); pasB <= (n_cnt != 0) & (nsB != IDLE);", "pasA <= (n_cnt != 0) & ((nsA == PRE) | (nsA == RUN)); pasB <= (n_cnt != 0) & ((nsB == PRE) | (nsB == RUN));")
rep("        if (wsA[k]) A[k]", "        if (cl[0]) A[k] <= 32'd0;\n        else if (wsA[k]) A[k]")
rep("        if (wsB[k]) B[k]", "        if (cl[1]) B[k] <= 32'd0;\n        else if (wsB[k]) B[k]")
open("gen.py", "w").write(s)
