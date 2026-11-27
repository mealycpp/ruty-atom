# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
# atom_seq v1 -> v2: the template is written through two shared ports (v1 had a variable mux on every lane: 11,012 LUT system, congested).
import shutil
p = "/mnt/c/Users/elhad/ruty/rtl_atom/atom_seq.v"
s = open(p, newline="").read().replace("\r\n", "\n")
shutil.copy(p, p.replace("atom_seq.v", "atom_seq_v1.v"))
a = s.index("  // ---------------- chain: the state of the next step, in one clock ----------------")
head = s[:a]
def rep(h, old, new):
    assert h.count(old) == 1, old
    return h.replace(old, new)
head = rep(head, "// in ONE clock, until that byte equals cend. Only the last digest leaves the atom.",
"// from the template, until that byte equals cend. Only the last digest leaves the atom. The template has two shared write ports, so a\n// step costs 2 clocks on top of the rounds for one chain and 3 for two chains.")
head = rep(head, "  localparam S_WAIT = 0, S_CLR = 1, S_ABS = 2, S_RUN = 3, S_SQ = 4;", "  localparam S_WAIT = 0, S_CLR = 1, S_ABS = 2, S_RUN = 3, S_SQ = 4, S_FB = 5, S_RL = 6;")
head = rep(head, "  reg [2:0] st; reg [23:0] jobs, rem; reg [4:0] i, rnd; reg dpad, sq; reg [15:0] ow; reg [7:0] c0, c1;", "  reg [2:0] st; reg [23:0] jobs, rem; reg [4:0] i, rnd; reg dpad, sq; reg [15:0] ow; reg [7:0] c0, c1; reg [63:0] tc;")
tail = r'''  // ---------------- chain: the template T, written through two shared ports ----------------
  function [63:0] setb(input [63:0] d, input [2:0] b, input [7:0] val); begin setb = (d & ~(64'hff << {b, 3'b000})) | ({56'd0, val} << {b, 3'b000}); end endfunction
  wire [7:0] c0n = c0 + 8'd1, c1n = c1 + 8'd1;
  wire [63:0] ctrw = dual ? setb(setb(tc, {1'b0, cb[1:0]}, c0n), {1'b1, cb[1:0]}, c1n) : setb(tc, cb, c0n);   // counter lane of the next step
  wire more = iter & (c0n != cend);
  wire fresh = (ow == wpo) & (i == 5'd0);
  wire t_abs = absgo & iter;                               // first pass: keep the padded message
  wire t_ctr = (st == S_RUN) & iter & (rnd == first);      // while the rounds run: counter of the next step
  wire t_fb0 = (st == S_SQ) & more & fresh;                // digest lanes 0,1 back into the template
  wire t_fb1 = (st == S_FB);                               // digest lanes 2,3 (two chains)
  wire we0 = t_abs | t_ctr | t_fb0 | t_fb1;
  wire [4:0] x0 = t_abs ? i : t_ctr ? cw : t_fb0 ? fbw : fbw + 5'd2;
  wire [63:0] d0 = t_abs ? aw : t_ctr ? ctrw : t_fb0 ? A[0] : A[2];
  wire we1 = (t_fb0 & (nfb > 3'd1)) | (t_fb1 & (nfb > 3'd3));
  wire [4:0] x1 = t_fb0 ? fbw + 5'd1 : fbw + 5'd3;
  wire [63:0] d1 = t_fb0 ? A[1] : A[3];
  integer k;
  always @(posedge clk) begin
    opush <= 1'b0;
    for (k = 0; k < 25; k = k + 1) begin
      if (st == S_CLR) T[k] <= 64'd0; else if (we1 && x1 == k) T[k] <= d1; else if (we0 && x0 == k) T[k] <= d0;
      if (st == S_CLR) A[k] <= 64'd0; else if (st == S_RUN) A[k] <= N[k]; else if (st == S_RL) A[k] <= T[k]; else if (absgo && i == k) A[k] <= A[k] ^ aw;
    end
    if (rst) begin st <= S_WAIT; idle <= 1'b1; end
    else case (st)
      S_WAIT: begin idle <= 1'b1; if (go) begin jobs <= njobs; idle <= 1'b0; st <= (njobs == 24'd0) ? S_WAIT : S_CLR; end end
      S_CLR:  begin rem <= len; i <= 5'd0; dpad <= 1'b0; sq <= 1'b0; ow <= wpo; st <= S_ABS; end
      S_ABS:  if (absgo) begin
                if (iter & need & (i == cw)) begin tc <= aw; c0 <= idata >> {dual ? {1'b0, cb[1:0]} : cb, 3'b000}; c1 <= idata >> {1'b1, cb[1:0], 3'b000}; end
                rem <= rem - {20'd0, v}; if (putd) dpad <= 1'b1;
                if (lastlane) begin rnd <= first; st <= S_RUN; end else i <= (dpad | putd) ? rate - 5'd1 : i + 5'd1;
              end
      S_RUN:  begin rnd <= rnd + 5'd1;
                if (rnd == last) begin i <= 5'd0; if (sq | dpad) begin sq <= 1'b1; st <= S_SQ; end else st <= S_ABS; end end
      S_SQ:   if (more & fresh) begin c0 <= c0n; c1 <= c1n; st <= (nfb > 3'd2) ? S_FB : S_RL; end      // chain: next step
              else if (~ofull & ~opush) begin
                odata <= A[i]; opush <= 1'b1; ow <= ow - 16'd1;
                if (ow == 16'd1) begin jobs <= jobs - 24'd1; st <= (jobs == 24'd1) ? S_WAIT : S_CLR; end
                else if (lastlane) begin rnd <= first; st <= S_RUN; end else i <= i + 5'd1;
              end
      S_FB:   st <= S_RL;
      S_RL:   begin rnd <= first; st <= S_RUN; end
      default: st <= S_WAIT;
    endcase
  end
endmodule
'''
open(p, "w", newline="\n").write(head + tail)
import hashlib; print(hashlib.md5((head + tail).encode()).hexdigest(), "want 2d201f1b3ea20f1e85b04c8d2a1db9a9")
