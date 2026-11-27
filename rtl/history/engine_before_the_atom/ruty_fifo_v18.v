// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_fifo: two-clock FIFO (gray-code pointers, two-flop synchronisers), registered output with valid/ready.
module ruty_fifo #(parameter W = 64, parameter AW = 6) (
    input wclk, input wrst, input [W-1:0] din, input push, output full,
    input rclk, input rrst, output reg [W-1:0] dout, output reg valid, input ready);
  reg [W-1:0] mem [0:(1<<AW)-1];
  reg [AW:0] wbin, wgray, rbin, rgray;
  (* ASYNC_REG = "TRUE" *) reg [AW:0] rg1, rg2, wg1, wg2;
  wire [AW:0] wbin_n = wbin + 1'b1, wgray_n = wbin_n ^ (wbin_n >> 1);
  assign full = (wgray == {~rg2[AW:AW-1], rg2[AW-2:0]});
  always @(posedge wclk) begin
    if (wrst) begin wbin <= 0; wgray <= 0; rg1 <= 0; rg2 <= 0; end
    else begin rg1 <= rgray; rg2 <= rg1;
      if (push & ~full) begin mem[wbin[AW-1:0]] <= din; wbin <= wbin_n; wgray <= wgray_n; end end
  end
  wire [AW:0] rbin_n = rbin + 1'b1, rgray_n = rbin_n ^ (rbin_n >> 1);
  wire have = (rgray != wg2), load = have & (~valid | ready);
  always @(posedge rclk) begin
    if (rrst) begin rbin <= 0; rgray <= 0; wg1 <= 0; wg2 <= 0; valid <= 1'b0; end
    else begin wg1 <= wgray; wg2 <= wg1;
      if (load) begin dout <= mem[rbin[AW-1:0]]; valid <= 1'b1; rbin <= rbin_n; rgray <= rgray_n; end
      else if (ready) valid <= 1'b0; end
  end
endmodule
