// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_fifo: two-clock FIFO (gray-code pointers, two-flop synchronisers), registered output with valid/ready.
// wcount: words in the FIFO as the writer sees them (never too low). rcount: words the reader can take (never too high).
module ruty_fifo #(parameter W = 64, parameter AW = 6) (
    input wclk, input wrst, input [W-1:0] din, input push, output full, output [AW:0] wcount,
    input rclk, input rrst, output reg [W-1:0] dout, output reg valid, input ready, output [AW+1:0] rcount);
  reg [W-1:0] mem [0:(1<<AW)-1];
  reg [AW:0] wbin, wgray, rbin, rgray;
  (* ASYNC_REG = "TRUE" *) reg [AW:0] rg1, rg2, wg1, wg2;
  function [AW:0] g2b(input [AW:0] g); integer k; begin g2b[AW] = g[AW]; for (k = AW - 1; k >= 0; k = k - 1) g2b[k] = g2b[k+1] ^ g[k]; end endfunction
  reg [AW:0] rb_w, wb_r; assign wcount = wbin - rb_w; assign rcount = {1'b0, wb_r - rbin} + valid;
  wire [AW:0] wbin_n = wbin + 1'b1, wgray_n = wbin_n ^ (wbin_n >> 1);
  assign full = (wgray == {~rg2[AW:AW-1], rg2[AW-2:0]});
  always @(posedge wclk) begin
    if (wrst) begin wbin <= 0; wgray <= 0; rg1 <= 0; rg2 <= 0; rb_w <= 0; end
    else begin rg1 <= rgray; rg2 <= rg1; rb_w <= g2b(rg2);
      if (push & ~full) begin mem[wbin[AW-1:0]] <= din; wbin <= wbin_n; wgray <= wgray_n; end end
  end
  wire [AW:0] rbin_n = rbin + 1'b1, rgray_n = rbin_n ^ (rbin_n >> 1);
  wire have = (rgray != wg2), load = have & (~valid | ready);
  always @(posedge rclk) begin
    if (rrst) begin rbin <= 0; rgray <= 0; wg1 <= 0; wg2 <= 0; valid <= 1'b0; wb_r <= 0; end
    else begin wg1 <= wgray; wg2 <= wg1; wb_r <= g2b(wg2);
      if (load) begin dout <= mem[rbin[AW-1:0]]; valid <= 1'b1; rbin <= rbin_n; rgray <= rgray_n; end
      else if (ready) valid <= 1'b0; end
  end
endmodule
