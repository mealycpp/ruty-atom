// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// Timing wrapper: a register on every input and output so that all paths of the engine are register to register.
module wrap(input clk, input rst, input dual, input [1:0] wr, input wxor, input [4:0] widx, input [63:0] wdata,
    input [4:0] ridx, output reg [63:0] rdata, input [1:0] start, input [4:0] first0, input [4:0] last0,
    input [4:0] first1, input [4:0] last1, input [1:0] clr, output reg [1:0] busy, output reg [1:0] err);
  reg rst_r, dual_r, wxor_r; reg [1:0] wr_r, start_r; reg [4:0] widx_r, ridx_r, f0, l0, f1, l1; reg [63:0] wdata_r;
  wire [63:0] rd; wire [1:0] bz, er; reg [1:0] clr_r;
  always @(posedge clk) begin
    rst_r <= rst; dual_r <= dual; wxor_r <= wxor; wr_r <= wr; start_r <= start; widx_r <= widx; ridx_r <= ridx;
    f0 <= first0; l0 <= last0; f1 <= first1; l1 <= last1; wdata_r <= wdata; rdata <= rd; busy <= bz; clr_r <= clr; err <= er;
  end
  `CORE core(.clk(clk), .rst(rst_r), .dual(dual_r), .wr(wr_r), .wxor(wxor_r), .widx(widx_r), .wdata(wdata_r),
    .ridx(ridx_r), .rdata(rd), .start(start_r), .first0(f0), .last0(l0), .first1(f1), .last1(l1), .busy(bz), .qfull(), .clr(clr_r), .err(er));
endmodule
