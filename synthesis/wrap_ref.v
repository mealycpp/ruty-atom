// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// Timing wrapper for the fixed reference: a register on every input and output, as for the RUTY engine.
module wrap(input clk, input rst, input go, input [511:0] tpl, input [7:0] cend, output reg busy, output reg done, output reg [127:0] dig);
  reg rst_r, go_r; reg [511:0] tpl_r; reg [7:0] cend_r; wire bz, dn; wire [127:0] dg;
  always @(posedge clk) begin rst_r <= rst; go_r <= go; tpl_r <= tpl; cend_r <= cend; busy <= bz; done <= dn; dig <= dg; end
  `CORE core(.clk(clk), .rst(rst_r), .go(go_r), .tpl(tpl_r), .cend(cend_r), .busy(bz), .done(dn), .dig(dg));
endmodule
