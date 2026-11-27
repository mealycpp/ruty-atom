// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_seq: sponge sequencer in the engine clock domain. Runs a batch of equal-shaped jobs on one RUTY engine:
// clear, absorb (message words from the input FIFO, padding made here), permute, squeeze into the output FIFO.
// One 64-bit word = one lane. dual=0: the word is 8 message bytes of one job. dual=1: the word carries 4 bytes of each of
// two jobs (low half = job on core 0, high half = job on core 1), both of the same length.
module ruty_seq(input clk, input rst, input go,
    input [23:0] njobs, input [23:0] len, input [7:0] dom, input [15:0] olen, input [4:0] first, input [4:0] last,
    input dual, input [4:0] rate,                                   // rate in lanes
    input [63:0] idata, input ivalid, output iready, output reg [63:0] odata, output reg opush, input ofull,
    output reg idle, output [1:0] eerr);
  localparam S_WAIT = 0, S_CLR = 1, S_ABS = 2, S_GO = 3, S_GAP = 4, S_RUN = 5, S_SQ = 6, S_SQ1 = 7, S_SQ2 = 8, S_SQ3 = 9;
  reg [3:0] st; reg [23:0] jobs, rem; reg [4:0] i; reg dpad, sq; reg [15:0] ow; reg [1:0] gap;
  reg [1:0] wr, clr, start; reg [4:0] widx, ridx; reg [63:0] wdata; wire [63:0] lane; wire [1:0] busy, qfull;
  ruty_core core(.clk(clk), .rst(rst), .dual(dual), .wr(wr), .wxor(1'b1), .widx(widx), .wdata(wdata), .clr(clr), .ridx(ridx), .rdata(lane),
                 .start(start), .first0(first), .last0(last), .first1(first), .last1(last), .busy(busy), .qfull(qfull), .err(eerr));
  wire [3:0] nb = dual ? 4'd4 : 4'd8;
  wire [15:0] wpo = dual ? ((olen + 16'd3) >> 2) : ((olen + 16'd7) >> 3);        // output words per job
  wire need = (rem != 24'd0);
  wire [3:0] v = (rem >= {20'd0, nb}) ? nb : rem[3:0];                           // message bytes in this lane
  wire putd = (v < nb) & ~dpad;
  wire lastlane = (i == rate - 5'd1);
  wire fin = (dpad | putd) & lastlane;
  // byte masks and padding. One 32-bit half is handled by half32; a 64-bit lane is two halves with the byte count split.
  function [31:0] half32(input [31:0] d, input [2:0] nv, input pd, input [7:0] dm, input f); reg [31:0] m; begin
    m = (nv >= 3'd4) ? 32'hffffffff : ~(32'hffffffff << {nv, 3'b000});
    half32 = (d & m) ^ ((pd && nv < 3'd4) ? ({24'd0, dm} << {nv, 3'b000}) : 32'd0) ^ (f ? 32'h80000000 : 32'd0); end endfunction
  wire [2:0] vlo = dual ? v[2:0] : ((v >= 4'd4) ? 3'd4 : v[2:0]);
  wire [2:0] vhi = dual ? v[2:0] : ((v >= 4'd4) ? v[2:0] - 3'd4 : 3'd0);
  wire [63:0] din = need ? idata : 64'd0;
  wire [31:0] plo = half32(din[31:0],  vlo, putd & (dual | (v < 4'd4)),  dom, fin & dual);
  wire [31:0] phi = half32(din[63:32], vhi, putd & (dual | (v >= 4'd4)), dom, fin);
  wire absgo = (st == S_ABS) & (~need | ivalid);
  assign iready = (st == S_ABS) & need;
  always @(posedge clk) begin
    wr <= 2'b00; clr <= 2'b00; start <= 2'b00; opush <= 1'b0;
    if (rst) begin st <= S_WAIT; idle <= 1'b1; end
    else case (st)
      S_WAIT: begin idle <= 1'b1; if (go) begin jobs <= njobs; idle <= 1'b0; st <= (njobs == 24'd0) ? S_WAIT : S_CLR; end end
      S_CLR:  begin clr <= 2'b11; rem <= len; i <= 5'd0; dpad <= 1'b0; sq <= 1'b0; ow <= wpo; st <= S_ABS; end
      S_ABS:  if (absgo) begin
                wr <= 2'b11; widx <= i; wdata <= {phi, plo}; rem <= rem - {20'd0, v}; if (putd) dpad <= 1'b1;
                if (lastlane) st <= S_GO; else i <= (dpad | putd) ? rate - 5'd1 : i + 5'd1;      // zero lanes need no write
              end
      S_GO:   begin start <= dual ? 2'b11 : 2'b01; gap <= 2'd0; st <= S_GAP; end
      S_GAP:  begin gap <= gap + 2'd1; if (gap == 2'd3) st <= S_RUN; end                          // let busy rise
      S_RUN:  if (busy == 2'b00) begin i <= 5'd0; if (dpad) begin sq <= 1'b1; st <= S_SQ; end else st <= S_ABS; end
      S_SQ:   begin ridx <= i; st <= S_SQ1; end
      S_SQ1:  st <= S_SQ2;
      S_SQ2:  begin odata <= lane; st <= S_SQ3; end
      S_SQ3:  if (~ofull) begin
                opush <= 1'b1; ow <= ow - 16'd1;
                if (ow == 16'd1) begin jobs <= jobs - 24'd1; st <= (jobs == 24'd1) ? S_WAIT : S_CLR; end
                else if (lastlane) st <= S_GO; else begin i <= i + 5'd1; st <= S_SQ; end
              end
      default: st <= S_WAIT;
    endcase
  end
endmodule
