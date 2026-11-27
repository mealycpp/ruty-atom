// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// atom_core: the round-per-clock morphing atom. One Keccak round per clock on 25 lanes of 64 bits.
//   dual = 0 : one Keccak-p[1600] state.        dual = 1 : two independent Keccak-p[800] states (low and high half of every lane).
// Only three things depend on the mode: the wrap of the theta rotation, the rho rotations, and the half of the round constant.
// Rounds first..last (SHAKE 0..23, TurboSHAKE 12..23, MiniSHAKE 10..21). The chain loop is inside: template seed(16) || ADRS(32) ||
// value(16), hash address in byte 47, 16-byte digest written straight back into the value, one clock between steps.
//   dual = 0 : one chain,  template in tpl lanes 0..7.   SHAKE256 padding (rate 17 lanes).
//   dual = 1 : two chains, templates interleaved in tpl lanes 0..15 (low half core 0, high half core 1). MiniSHAKE128 padding (rate 17 half-lanes).
// Simulated against the reference in both widths: 18 chain cases, 0 mismatches; 25 clocks per SHAKE256 step, 13 per two MiniSHAKE128 steps.
module atom_core (input clk, input rst, input go, input dual, input [4:0] first, input [4:0] last, input [7:0] dom,
                  input [1023:0] tpl, input [7:0] cend, output reg busy, output reg done, output [255:0] dig);
  reg [63:0] A [0:24]; reg [4:0] rnd; reg run; reg [7:0] cnt; reg [1023:0] t;
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
  function integer rho(input integer i);
    case (i) 0:rho=0; 1:rho=1; 2:rho=62; 3:rho=28; 4:rho=27; 5:rho=36; 6:rho=44; 7:rho=6; 8:rho=55; 9:rho=20; 10:rho=3; 11:rho=10; 12:rho=43;
      13:rho=25; 14:rho=39; 15:rho=41; 16:rho=45; 17:rho=15; 18:rho=21; 19:rho=8; 20:rho=18; 21:rho=2; 22:rho=61; 23:rho=56; default:rho=14; endcase
  endfunction
  wire [63:0] C [0:4]; wire [63:0] D [0:4]; wire [63:0] T [0:24]; wire [63:0] B [0:24]; wire [63:0] N [0:24];
  wire [63:0] rc64 = rc(rnd); wire [63:0] rcw = dual ? {rc64[31:0], rc64[31:0]} : rc64;
  genvar x, y, i, j;
  generate
    for (x = 0; x < 5; x = x + 1) begin : gc
      assign C[x] = A[x] ^ A[x+5] ^ A[x+10] ^ A[x+15] ^ A[x+20];
      wire [63:0] c1 = C[(x+1)%5];
      assign D[x] = C[(x+4)%5] ^ {c1[62:32], (dual ? c1[63] : c1[31]), c1[30:0], (dual ? c1[31] : c1[63])};
    end
    for (i = 0; i < 25; i = i + 1) begin : gt
      assign T[i] = A[i] ^ D[i%5];
    end
    for (x = 0; x < 5; x = x + 1) begin : gx
      for (y = 0; y < 5; y = y + 1) begin : gy
        localparam integer S = x + 5*y; localparam integer R = rho(S); localparam integer Q = y + 5*((2*x+3*y)%5);
        for (j = 0; j < 64; j = j + 1) begin : gb
          localparam integer S64 = (j - R + 128) % 64;
          localparam integer S32 = (j < 32) ? ((j - (R % 32) + 64) % 32) : (32 + ((j - 32 - (R % 32) + 64) % 32));
          if (S64 == S32) begin : same assign B[Q][j] = T[S][S64]; end
          else begin : diff assign B[Q][j] = dual ? T[S][S32] : T[S][S64]; end
        end
      end
    end
    for (i = 0; i < 25; i = i + 1) begin : gn
      wire [63:0] chi = B[i] ^ (~B[(i%5+1)%5 + 5*(i/5)] & B[(i%5+2)%5 + 5*(i/5)]);
      assign N[i] = (i == 0) ? chi ^ rcw : chi;
    end
  endgenerate
  assign dig = {A[3], A[2], A[1], A[0]};
  integer k;
  always @(posedge clk) begin
    done <= 1'b0;
    if (rst) begin busy <= 1'b0; run <= 1'b0; end
    else if (go & ~busy) begin t <= tpl; cnt <= dual ? tpl[64*11+31 -: 8] : tpl[64*5+63 -: 8]; busy <= 1'b1; run <= 1'b0; end
    else if (busy & ~run) begin                      // one clock: load the template, the counter and the padding
      for (k = 0; k < 25; k = k + 1) A[k] <= 64'd0;
      for (k = 0; k < 8; k = k + 1) A[k] <= t[64*k +: 64];
      if (dual) begin
        for (k = 8; k < 16; k = k + 1) A[k] <= t[64*k +: 64];
        A[11] <= {cnt, t[64*11+32 +: 24], cnt, t[64*11 +: 24]}; A[16] <= {8'h80, 16'h0, dom, 8'h80, 16'h0, dom};
      end else begin
        A[5] <= {cnt, t[64*5 +: 56]}; A[8] <= {56'h0, dom}; A[16] <= 64'h8000000000000000;
      end
      rnd <= first; run <= 1'b1;
    end
    else if (run) begin
      for (k = 0; k < 25; k = k + 1) A[k] <= N[k];
      rnd <= rnd + 5'd1;
      if (rnd == last) begin run <= 1'b0; cnt <= cnt + 8'd1;
        if (dual) t[64*12 +: 256] <= {N[3], N[2], N[1], N[0]}; else t[64*6 +: 128] <= {N[1], N[0]};
        if (cnt + 8'd1 == cend) begin busy <= 1'b0; done <= 1'b1; end end
    end
  end
endmodule
