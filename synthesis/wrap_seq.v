// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// Timing wrapper for the atom with its sequencer (module ruty_seq): a register on every input and output.
module wrap(input clk, input rst, input go, input [23:0] njobs, input [23:0] len, input [7:0] dom, input [15:0] olen, input [4:0] first, input [4:0] last,
            input dual, input [4:0] rate, input iter, input [4:0] cw, input [2:0] cb, input [4:0] fbw, input [7:0] cend,
            input [63:0] idata, input ivalid, output reg iready, output reg [63:0] odata, output reg opush, input ofull, output reg idle);
  reg rst_r, go_r, dual_r, iter_r, iv_r, of_r; reg [23:0] nj_r, len_r; reg [7:0] dom_r, cend_r; reg [15:0] ol_r; reg [4:0] f_r, l_r, rate_r, cw_r, fbw_r; reg [2:0] cb_r; reg [63:0] id_r;
  wire ir, op, il; wire [63:0] od;
  always @(posedge clk) begin rst_r <= rst; go_r <= go; dual_r <= dual; iter_r <= iter; iv_r <= ivalid; of_r <= ofull; nj_r <= njobs; len_r <= len; dom_r <= dom; cend_r <= cend;
    ol_r <= olen; f_r <= first; l_r <= last; rate_r <= rate; cw_r <= cw; fbw_r <= fbw; cb_r <= cb; id_r <= idata; iready <= ir; odata <= od; opush <= op; idle <= il; end
  ruty_seq core(.clk(clk), .rst(rst_r), .go(go_r), .njobs(nj_r), .len(len_r), .dom(dom_r), .olen(ol_r), .first(f_r), .last(l_r), .dual(dual_r), .rate(rate_r),
                .iter(iter_r), .cw(cw_r), .cb(cb_r), .fbw(fbw_r), .cend(cend_r), .idata(id_r), .ivalid(iv_r), .iready(ir), .odata(od), .opush(op), .ofull(of_r), .idle(il), .eerr());
endmodule
