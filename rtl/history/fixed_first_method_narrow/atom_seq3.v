// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// atom_seq (module ruty_seq, drop-in for the folded sequencer + engine): the round-per-clock morphing atom with its sponge sequencer.
// One Keccak round per clock on 25 lanes of 64 bits. dual=0: one Keccak-p[1600]. dual=1: two independent Keccak-p[800] (low / high half
// of every lane). Only the theta wrap, the rho rotations and the half of the round constant depend on the mode.
// Batch of equal-shaped jobs: absorb one word per clock from the input FIFO (padding made here), permute rounds first..last, squeeze.
// One 64-bit word = one lane; in two-core mode a word carries 4 bytes of each of two jobs.
// Chain mode (iter=1, single-block message): the padded message is kept as a template. After each hash the digest lanes are written
// back at lane fbw, one byte of lane cw (byte cb; in two-core mode the same byte of each half) is counted up, and the state is reloaded
// from the template, until that byte equals cend. Only the last digest leaves the atom. The template has two shared write ports, so a
// step costs 2 clocks on top of the rounds for one chain and 3 for two chains. Every control of the 1600-bit registers comes from a
// flip-flop, and absorbed words are registered once, so the round is the longest path.
module ruty_seq(input clk, input rst, input go,
    input [23:0] njobs, input [23:0] len, input [7:0] dom, input [15:0] olen, input [4:0] first, input [4:0] last,
    input dual, input [4:0] rate,
    input iter, input [4:0] cw, input [2:0] cb, input [4:0] fbw, input [7:0] cend,
    input [63:0] idata, input ivalid, output iready, output reg [63:0] odata, output reg opush, input ofull,
    output reg idle, output [1:0] eerr);
  localparam S_WAIT = 0, S_CLR = 1, S_ABS = 2, S_GO = 3, S_RUN = 4, S_SQ = 5, S_FB0 = 6, S_FB1 = 7, S_RL = 8;
  assign eerr = 2'b00;
  reg [63:0] A [0:24]; reg [63:0] T [0:24];
  reg [3:0] st; reg [23:0] jobs, rem; reg [4:0] i, rnd; reg dpad, sq; reg [15:0] ow; reg [7:0] c0, c1; reg [63:0] tc;
  // ---------------- the round ----------------
  function [63:0] rc(input [4:0] r);
    case (r)
      0: rc=64'h0000000000000001; 1: rc=64'h0000000000008082; 2: rc=64'h800000000000808A; 3: rc=64'h8000000080008000;
      4: rc=64'h000000000000808B; 5: rc=64'h0000000080000001; 6: rc=64'h8000000080008081; 7: rc=64'h8000000000008009;
      8: rc=64'h000000000000008A; 9: rc=64'h0000000000000088; 10: rc=64'h0000000080008009; 11: rc=64'h000000008000000A;
      12: rc=64'h000000008000808B; 13: rc=64'h800000000000008B; 14: rc=64'h8000000000008089; 15: rc=64'h8000000000008003;
      16: rc=64'h8000000000008002; 17: rc=64'h8000000000000080; 18: rc=64'h000000000000800A; 19: rc=64'h800000008000000A;
      20: rc=64'h8000000080008081; 21: rc=64'h8000000000008080; 22: rc=64'h0000000080000001; default: rc=64'h8000000080008008;
    endcase
  endfunction
  function integer rho(input integer n);
    case (n) 0:rho=0; 1:rho=1; 2:rho=62; 3:rho=28; 4:rho=27; 5:rho=36; 6:rho=44; 7:rho=6; 8:rho=55; 9:rho=20; 10:rho=3; 11:rho=10; 12:rho=43;
      13:rho=25; 14:rho=39; 15:rho=41; 16:rho=45; 17:rho=15; 18:rho=21; 19:rho=8; 20:rho=18; 21:rho=2; 22:rho=61; 23:rho=56; default:rho=14; endcase
  endfunction
  wire [63:0] C [0:4]; wire [63:0] D [0:4]; wire [63:0] TH [0:24]; wire [63:0] B [0:24]; wire [63:0] N [0:24];
  wire [63:0] rc64 = rc(rnd); wire [63:0] rcw = dual ? {rc64[31:0], rc64[31:0]} : rc64;
  genvar x, y, g, j;
  generate
    for (x = 0; x < 5; x = x + 1) begin : gc
      assign C[x] = A[x] ^ A[x+5] ^ A[x+10] ^ A[x+15] ^ A[x+20];
      wire [63:0] c1w = C[(x+1)%5];
      assign D[x] = C[(x+4)%5] ^ {c1w[62:32], (dual ? c1w[63] : c1w[31]), c1w[30:0], (dual ? c1w[31] : c1w[63])};
    end
    for (g = 0; g < 25; g = g + 1) begin : gt
      assign TH[g] = A[g] ^ D[g%5];
    end
    for (x = 0; x < 5; x = x + 1) begin : gx
      for (y = 0; y < 5; y = y + 1) begin : gy
        localparam integer S = x + 5*y; localparam integer R = rho(S); localparam integer Q = y + 5*((2*x+3*y)%5);
        for (j = 0; j < 64; j = j + 1) begin : gb
          localparam integer S64 = (j - R + 128) % 64;
          localparam integer S32 = (j < 32) ? ((j - (R % 32) + 64) % 32) : (32 + ((j - 32 - (R % 32) + 64) % 32));
          if (S64 == S32) begin : same assign B[Q][j] = TH[S][S64]; end
          else begin : diff assign B[Q][j] = dual ? TH[S][S32] : TH[S][S64]; end
        end
      end
    end
    for (g = 0; g < 25; g = g + 1) begin : gn
      wire [63:0] chi = B[g] ^ (~B[(g%5+1)%5 + 5*(g/5)] & B[(g%5+2)%5 + 5*(g/5)]);
      assign N[g] = (g == 0) ? chi ^ rcw : chi;
    end
  endgenerate
  // ---------------- absorb: byte masks and padding (as in the folded sequencer) ----------------
  wire [3:0] nb = dual ? 4'd4 : 4'd8;
  wire [15:0] wpo_c = dual ? ((olen + 16'd3) >> 2) : ((olen + 16'd7) >> 3);
  reg [15:0] wpo; wire [2:0] nfb = wpo[2:0];
  wire need = (rem != 24'd0);
  wire [3:0] v = (rem >= {20'd0, nb}) ? nb : rem[3:0];
  wire putd = (v < nb) & ~dpad;
  wire lastlane = (i == rate - 5'd1);
  wire fin = (dpad | putd) & lastlane;
  function [31:0] half32(input [31:0] d, input [2:0] nv, input pd, input [7:0] dm, input f); reg [31:0] m; begin
    m = (nv >= 3'd4) ? 32'hffffffff : ~(32'hffffffff << {nv, 3'b000});
    half32 = (d & m) ^ ((pd && nv < 3'd4) ? ({24'd0, dm} << {nv, 3'b000}) : 32'd0) ^ (f ? 32'h80000000 : 32'd0); end endfunction
  wire [2:0] vlo = dual ? v[2:0] : ((v >= 4'd4) ? 3'd4 : v[2:0]);
  wire [2:0] vhi = dual ? v[2:0] : ((v >= 4'd4) ? v[2:0] - 3'd4 : 3'd0);
  wire [63:0] din = need ? idata : 64'd0;
  wire [31:0] plo = half32(din[31:0],  vlo, putd & (dual | (v < 4'd4)),  dom, fin & dual);
  wire [31:0] phi = half32(din[63:32], vhi, putd & (dual | (v >= 4'd4)), dom, fin);
  wire [63:0] aw = {phi, plo};
  wire absgo = (st == S_ABS) & (~need | ivalid);
  assign iready = (st == S_ABS) & need;
  // ---------------- registered controls of the state A and of the template T ----------------
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
