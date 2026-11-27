// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// ref_chain: the fixed reference. One Keccak-f[1600] round per clock, SHAKE256, and a Winternitz chain loop with one clock between steps
// (25 clocks per step). It does one thing: template seed(16) || ADRS(32) || value(16), hash address in byte 47, 16-byte digest fed back.
// No width change, no other function, no bus master. Used only to price the flexibility of the RUTY atom on the same device.
// Checked in simulation against the FIPS 205 chain vector (digest 3a666d7625b1d02f484e003f1df7aa4d after 15 steps, 375 clocks).
module ref_chain (input clk, input rst, input go, input [511:0] tpl, input [7:0] cend, output reg busy, output reg done, output [127:0] dig);
  reg [63:0] A [0:24]; reg [4:0] rnd; reg run; reg [7:0] cnt; reg [511:0] t;
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
  function [5:0] rho(input integer i);
    case (i) 0:rho=0; 1:rho=1; 2:rho=62; 3:rho=28; 4:rho=27; 5:rho=36; 6:rho=44; 7:rho=6; 8:rho=55; 9:rho=20; 10:rho=3; 11:rho=10; 12:rho=43;
      13:rho=25; 14:rho=39; 15:rho=41; 16:rho=45; 17:rho=15; 18:rho=21; 19:rho=8; 20:rho=18; 21:rho=2; 22:rho=61; 23:rho=56; default:rho=14; endcase
  endfunction
  wire [63:0] C [0:4]; wire [63:0] D [0:4]; wire [63:0] T [0:24]; wire [63:0] B [0:24]; wire [63:0] N [0:24];
  genvar x, y, i;
  generate
    for (x = 0; x < 5; x = x + 1) begin : gc
      assign C[x] = A[x] ^ A[x+5] ^ A[x+10] ^ A[x+15] ^ A[x+20];
      assign D[x] = C[(x+4)%5] ^ {C[(x+1)%5][62:0], C[(x+1)%5][63]};
    end
    for (i = 0; i < 25; i = i + 1) begin : gt
      assign T[i] = A[i] ^ D[i%5];
    end
    for (x = 0; x < 5; x = x + 1) begin : gx
      for (y = 0; y < 5; y = y + 1) begin : gy
        localparam integer S = x + 5*y; localparam integer R = rho(S);
        if (R == 0) begin : r0 assign B[y + 5*((2*x+3*y)%5)] = T[S]; end
        else begin : r1 assign B[y + 5*((2*x+3*y)%5)] = {T[S][63-R:0], T[S][63:64-R]}; end
      end
    end
    for (i = 0; i < 25; i = i + 1) begin : gn
      wire [63:0] chi = B[i] ^ (~B[(i%5+1)%5 + 5*(i/5)] & B[(i%5+2)%5 + 5*(i/5)]);
      assign N[i] = (i == 0) ? chi ^ rc(rnd) : chi;
    end
  endgenerate
  assign dig = {A[1], A[0]};
  integer k;
  always @(posedge clk) begin
    done <= 1'b0;
    if (rst) begin busy <= 1'b0; run <= 1'b0; end
    else if (go & ~busy) begin t <= tpl; cnt <= tpl[383:376]; busy <= 1'b1; run <= 1'b0; end
    else if (busy & ~run) begin                      // load one step: the template, the value, the padding
      for (k = 0; k < 25; k = k + 1) A[k] <= 64'd0;
      for (k = 0; k < 8; k = k + 1) A[k] <= t[64*k +: 64];
      A[5] <= {cnt, t[375:320]}; A[8] <= 64'h1F; A[16] <= 64'h8000000000000000; rnd <= 5'd0; run <= 1'b1;
    end
    else if (run) begin
      for (k = 0; k < 25; k = k + 1) A[k] <= N[k];
      rnd <= rnd + 5'd1;
      if (rnd == 5'd23) begin run <= 1'b0; cnt <= cnt + 8'd1; t[511:384] <= {N[1], N[0]};
        if (cnt + 8'd1 == cend) begin busy <= 1'b0; done <= 1'b1; end end
    end
  end
endmodule
