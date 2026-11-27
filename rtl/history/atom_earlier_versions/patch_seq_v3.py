# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# atom_seq v2 -> v3 (written to atom_seq3.v): every control of the 1600-bit registers comes from a flip-flop, absorbed words are registered once.
# v2 out of context: 7,442 LUT, about 350 MHz, longest path olen -> template write enable (9 levels).
p = "/mnt/c/Users/elhad/ruty/rtl_atom/atom_seq.v"
s = open(p, newline="").read().replace("\r\n", "\n")
a = s.index("  // ---------------- chain: the template T, written through two shared ports ----------------")
head = s[:a]
def rep(h, old, new):
    assert h.count(old) == 1, old
    return h.replace(old, new)
head = rep(head, "  localparam S_WAIT = 0, S_CLR = 1, S_ABS = 2, S_RUN = 3, S_SQ = 4, S_FB = 5, S_RL = 6;", "  localparam S_WAIT = 0, S_CLR = 1, S_ABS = 2, S_GO = 3, S_RUN = 4, S_SQ = 5, S_FB0 = 6, S_FB1 = 7, S_RL = 8;")
head = rep(head, "  reg [2:0] st; reg [23:0] jobs, rem;", "  reg [3:0] st; reg [23:0] jobs, rem;")
head = rep(head, "// step costs 2 clocks on top of the rounds for one chain and 3 for two chains.", "// step costs 2 clocks on top of the rounds for one chain and 3 for two chains. Every control of the 1600-bit registers comes from a\n// flip-flop, and absorbed words are registered once, so the round is the longest path.")
head = rep(head, "  wire [15:0] wpo = dual ? ((olen + 16'd3) >> 2) : ((olen + 16'd7) >> 3);\n  wire [2:0] nfb = wpo[2:0];", "  wire [15:0] wpo_c = dual ? ((olen + 16'd3) >> 2) : ((olen + 16'd7) >> 3);\n  reg [15:0] wpo; wire [2:0] nfb = wpo[2:0];")
tail = r'''  // ---------------- registered controls of the state A and of the template T ----------------
  reg f_clr, f_run, f_rl;                          // A <= 0, A <= round(A), A <= T
  reg wv; reg [4:0] wi; reg [63:0] awr;            // absorbed word, one clock late: A[wi] ^= awr
  reg p0v, p1v; reg [4:0] p0x, p1x; reg [1:0] p0s; reg p1s;      // the two write ports of T
  function [63:0] setb(input [63:0] d, input [2:0] b, input [7:0] val); begin setb = (d & ~(64'hff << {b, 3'b000})) | ({56'd0, val} << {b, 3'b000}); end endfunction
  wire [7:0] c0n = c0 + 8'd1, c1n = c1 + 8'd1;
  wire [63:0] ctrw = dual ? setb(setb(tc, {1'b0, cb[1:0]}, c0n), {1'b1, cb[1:0]}, c1n) : setb(tc, cb, c0n);   // counter lane of the next step
  wire more = iter & (c0n != cend);
  wire [63:0] d0 = (p0s == 2'd0) ? awr : (p0s == 2'd1) ? ctrw : (p0s == 2'd2) ? A[0] : A[2];
  wire [63:0] d1 = p1s ? A[3] : A[1];
  integer k;
  always @(posedge clk) begin
    opush <= 1'b0; wv <= 1'b0; p0v <= 1'b0; p1v <= 1'b0;
    for (k = 0; k < 25; k = k + 1) begin
      if (f_clr) T[k] <= 64'd0; else if (p1v && p1x == k) T[k] <= d1; else if (p0v && p0x == k) T[k] <= d0;
      if (f_clr) A[k] <= 64'd0; else if (f_run) A[k] <= N[k]; else if (f_rl) A[k] <= T[k]; else if (wv && wi == k) A[k] <= A[k] ^ awr;
    end
    if (rst) begin st <= S_WAIT; idle <= 1'b1; f_clr <= 1'b0; f_run <= 1'b0; f_rl <= 1'b0; end
    else case (st)
      S_WAIT: begin idle <= 1'b1; wpo <= wpo_c; if (go) begin jobs <= njobs; idle <= 1'b0; if (njobs != 24'd0) begin f_clr <= 1'b1; st <= S_CLR; end end end
      S_CLR:  begin f_clr <= 1'b0; rem <= len; i <= 5'd0; dpad <= 1'b0; sq <= 1'b0; ow <= wpo; st <= S_ABS; end
      S_ABS:  if (absgo) begin
                awr <= aw; wi <= i; wv <= 1'b1; if (iter) begin p0v <= 1'b1; p0x <= i; p0s <= 2'd0; end
                if (iter & need & (i == cw)) begin tc <= aw; c0 <= idata >> {dual ? {1'b0, cb[1:0]} : cb, 3'b000}; c1 <= idata >> {1'b1, cb[1:0], 3'b000}; end
                rem <= rem - {20'd0, v}; if (putd) dpad <= 1'b1;
                if (lastlane) st <= S_GO; else i <= (dpad | putd) ? rate - 5'd1 : i + 5'd1;
              end
      S_GO:   begin rnd <= first; f_run <= 1'b1; st <= S_RUN;                                    // the last absorbed word lands in this clock
                if (iter) begin p0v <= 1'b1; p0x <= cw; p0s <= 2'd1; end end                      // counter of the next step
      S_RUN:  begin rnd <= rnd + 5'd1;
                if (rnd == last) begin f_run <= 1'b0; i <= 5'd0;
                  if (~(sq | dpad)) st <= S_ABS;
                  else if (more) begin st <= S_FB0; p0v <= 1'b1; p0x <= fbw; p0s <= 2'd2; p1v <= (nfb > 3'd1); p1x <= fbw + 5'd1; p1s <= 1'b0; end
                  else begin sq <= 1'b1; st <= S_SQ; end end end
      S_FB0:  begin c0 <= c0n; c1 <= c1n;                                                        // digest lanes 0,1 go into T in this clock
                if (nfb > 3'd2) begin st <= S_FB1; p0v <= 1'b1; p0x <= fbw + 5'd2; p0s <= 2'd3; p1v <= (nfb > 3'd3); p1x <= fbw + 5'd3; p1s <= 1'b1; end
                else begin f_rl <= 1'b1; st <= S_RL; end end
      S_FB1:  begin f_rl <= 1'b1; st <= S_RL; end                                                // digest lanes 2,3
      S_RL:   begin f_rl <= 1'b0; f_run <= 1'b1; rnd <= first; st <= S_RUN; p0v <= 1'b1; p0x <= cw; p0s <= 2'd1; end
      S_SQ:   if (~ofull & ~opush) begin
                odata <= A[i]; opush <= 1'b1; ow <= ow - 16'd1;
                if (ow == 16'd1) begin jobs <= jobs - 24'd1; if (jobs == 24'd1) st <= S_WAIT; else begin f_clr <= 1'b1; st <= S_CLR; end end
                else if (lastlane) begin rnd <= first; f_run <= 1'b1; st <= S_RUN; end else i <= i + 5'd1;
              end
      default: st <= S_WAIT;
    endcase
  end
endmodule
'''
open(p.replace("atom_seq.v", "atom_seq3.v"), "w", newline="\n").write(head + tail)
import hashlib; print(hashlib.md5((head + tail).encode()).hexdigest(), "want c3e29aa94ded09ec8e43be120b6f95ba")
