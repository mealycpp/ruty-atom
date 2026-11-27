# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""gen.py v1 -> v2: registered per-half control, one pipeline stage on lane writes, request queue with its own round range."""
import os; os.chdir(os.path.dirname(os.path.abspath(__file__)))
s = open("gen_v1.py").read()
def rep(a, b, n=1):
    global s
    assert s.count(a) == n, (a, s.count(a)); s = s.replace(a, b)
rep("  reg [1:0] st0, st1, pend; reg th0, th1;", "  reg [1:0] st0, st1, pend; reg th0, th1; reg [1:0] n_st0, n_st1; reg n_th0, n_th1; reg [4:0] n_ri0, n_ri1; reg [1:0] take;\n  reg preA, preB, thA, thB, rotA, rotB, pasA, pasB, fp;        // per-half control, registered one clock ahead\n  reg [24:0] wsA, wsB; reg wx; reg [63:0] wd;                  // lane write, one pipeline stage")
rep("""  wire [1:0] sA = st0, sB = du ? st1 : st0;
  wire thA = th0, thB = du ? th1 : th0;
  wire rot = (cnt == 0), last = (cnt == {P});
  wire rotA = rot & (sA == RUN), rotB = rot & (sB == RUN), pasA = ~rot & (sA != IDLE), pasB = ~rot & (sB != IDLE);
""", """  wire rot = (cnt == 0), last = (cnt == {P});
  wire [{CW-1}:0] n_cnt = last ? {CW}'d0 : cnt + {CW}'d1;
  always @* begin                                                        // slot boundary: private job control
    n_st0 = st0; n_th0 = th0; n_ri0 = ri0; n_st1 = st1; n_th1 = th1; n_ri1 = ri1; take = 2'b00;
    if (last) begin
      case (st0)
        PRE: begin n_st0 = RUN; n_ri0 = pf0; n_th0 = (pf0 != pl0); end
        RUN: if (ri0 != rl0) begin n_ri0 = ri0 + 5'd1; n_th0 = ((ri0 + 5'd1) != rl0); end
             else if (pend[0]) begin n_st0 = PRE; n_th0 = 1'b1; take[0] = 1'b1; end
             else begin n_st0 = IDLE; n_th0 = 1'b0; end
        default: if (pend[0]) begin n_st0 = PRE; n_th0 = 1'b1; take[0] = 1'b1; end
      endcase
      case (st1)
        PRE: begin n_st1 = RUN; n_ri1 = pf1; n_th1 = (pf1 != pl1); end
        RUN: if (ri1 != rl1) begin n_ri1 = ri1 + 5'd1; n_th1 = ((ri1 + 5'd1) != rl1); end
             else if (pend[1]) begin n_st1 = PRE; n_th1 = 1'b1; take[1] = 1'b1; end
             else begin n_st1 = IDLE; n_th1 = 1'b0; end
        default: if (pend[1]) begin n_st1 = PRE; n_th1 = 1'b1; take[1] = 1'b1; end
      endcase
    end
  end
  wire [1:0] nsA = n_st0, nsB = du ? n_st1 : n_st0;
""")
rep("wire [4:0] ciA = (cnt == 1) ? 5'd0 : cpA, ciB = (cnt == 1) ? 5'd0 : cpB;", "wire [4:0] ciA = fp ? 5'd0 : cpA, ciB = fp ? 5'd0 : cpB;")
rep("pc(sl_A[j], rcA[j], sA == PRE); assign s_B[j] = pc(sl_B[j], rcB[j], sB == PRE);", "pc(sl_A[j], rcA[j], preA); assign s_B[j] = pc(sl_B[j], rcB[j], preB);")
a = s.index("    if (rst) begin st0 <= IDLE;"); b = s.index("      if (rot) begin rcA")
s = s[:a] + """    wsA <= wr[0] ? (25'd1 << widx) : 25'd0; wsB <= wr[1] ? (25'd1 << widx) : 25'd0; wx <= wxor; wd <= wdata;
    if (rst) begin st0 <= IDLE; st1 <= IDLE; pend <= 2'b00; cnt <= 0; th0 <= 1'b0; th1 <= 1'b0;
      preA <= 1'b0; preB <= 1'b0; thA <= 1'b0; thB <= 1'b0; rotA <= 1'b0; rotB <= 1'b0; pasA <= 1'b0; pasB <= 1'b0; fp <= 1'b0; end
    else begin
      cnt <= n_cnt;                                                      // shared slot counter, free running
      st0 <= n_st0; th0 <= n_th0; ri0 <= n_ri0; st1 <= n_st1; th1 <= n_th1; ri1 <= n_ri1;
      // a start pulse is latched; the latched request is taken at a slot boundary when the core is free.
      // The round range is copied at the take, so a queued job may carry a different range.
      if (take[0]) begin pend[0] <= start[0]; pf0 <= qf0; pl0 <= ql0; if (start[0]) begin qf0 <= first0; ql0 <= last0; end end
      else if (start[0]) begin pend[0] <= 1'b1; qf0 <= first0; ql0 <= last0; end
      if (take[1]) begin pend[1] <= start[1] & du; pf1 <= qf1; pl1 <= ql1; if (start[1]) begin qf1 <= first1; ql1 <= last1; end end
      else if (start[1] & du) begin pend[1] <= 1'b1; qf1 <= first1; ql1 <= last1; end
      if (last && st0 == PRE) rl0 <= pl0;
      if (last && st1 == PRE) rl1 <= pl1;
      preA <= (nsA == PRE); preB <= (nsB == PRE); thA <= n_th0; thB <= du ? n_th1 : n_th0; fp <= (n_cnt == 1);
      rotA <= (n_cnt == 0) & (nsA == RUN); rotB <= (n_cnt == 0) & (nsB == RUN);
      pasA <= (n_cnt != 0) & (nsA != IDLE); pasB <= (n_cnt != 0) & (nsB != IDLE);
""" + s[b:]
rep("ri0, rl0, pf0, pl0, ri1, rl1, pf1, pl1;", "ri0, rl0, pf0, pl0, qf0, ql0, ri1, rl1, pf1, pl1, qf1, ql1;")
rep("        if (wr[0] && widx == k) A[k] <= wxor ? (A[k] ^ wdata[31:0]) : wdata[31:0];", "        if (wsA[k]) A[k] <= wx ? (A[k] ^ wd[31:0]) : wd[31:0];")
rep("        if (wr[1] && widx == k) B[k] <= wxor ? (B[k] ^ wdata[63:32]) : wdata[63:32];", "        if (wsB[k]) B[k] <= wx ? (B[k] ^ wd[63:32]) : wd[63:32];")
rep("reg [63:0] wd;                  // lane write, one pipeline stage reg [4:0] ri0", "reg [63:0] wd;                  // lane write, one pipeline stage\n  reg [4:0] ri0")
rep("    output [1:0] busy);", "    output [1:0] busy, output [1:0] qfull);                              // qfull: a request is waiting, do not pulse start")
rep("  assign busy = {{", "  assign qfull = pend;\n  assign busy = {{")
open("gen.py", "w").write(s)
