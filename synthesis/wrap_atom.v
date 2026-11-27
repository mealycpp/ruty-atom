// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// Timing wrapper for the round-per-clock morphing atom: a register on every input and output, as for the other engines.
module wrap(input clk, input rst, input go, input dual, input [4:0] first, input [4:0] last, input [7:0] dom, input [1023:0] tpl, input [7:0] cend,
            output reg busy, output reg done, output reg [255:0] dig);
  reg rst_r, go_r, dual_r; reg [4:0] f_r, l_r; reg [7:0] dom_r, cend_r; reg [1023:0] tpl_r; wire bz, dn; wire [255:0] dg;
  always @(posedge clk) begin rst_r <= rst; go_r <= go; dual_r <= dual; f_r <= first; l_r <= last; dom_r <= dom; tpl_r <= tpl; cend_r <= cend; busy <= bz; done <= dn; dig <= dg; end
  `CORE core(.clk(clk), .rst(rst_r), .go(go_r), .dual(dual_r), .first(f_r), .last(l_r), .dom(dom_r), .tpl(tpl_r), .cend(cend_r), .busy(bz), .done(dn), .dig(dg));
endmodule
