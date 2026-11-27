# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""v19: the bus master never stalls a burst. Reads are requested only against free space in the input FIFO (credits, several bursts
may be outstanding); a write burst is requested only when all its words are already in the output FIFO. Engines that share one
memory port therefore cannot block each other (v18 could deadlock: A's write burst waits for A's data, which waits behind B's stalled read)."""
import re
f = open("ruty_fifo.v").read()
f = f.replace("// ruty_fifo: two-clock FIFO (gray-code pointers, two-flop synchronisers), registered output with valid/ready.",
 "// ruty_fifo: two-clock FIFO (gray-code pointers, two-flop synchronisers), registered output with valid/ready.\n// wcount: words in the FIFO as the writer sees them (never too low). rcount: words the reader can take (never too high).")
f = f.replace("input wclk, input wrst, input [W-1:0] din, input push, output full,", "input wclk, input wrst, input [W-1:0] din, input push, output full, output [AW:0] wcount,")
f = f.replace("output reg [W-1:0] dout, output reg valid, input ready);", "output reg [W-1:0] dout, output reg valid, input ready, output [AW+1:0] rcount);")
f = f.replace("  wire [AW:0] wbin_n = wbin + 1'b1,", "  function [AW:0] g2b(input [AW:0] g); integer k; begin g2b[AW] = g[AW]; for (k = AW - 1; k >= 0; k = k - 1) g2b[k] = g2b[k+1] ^ g[k]; end endfunction\n"
  "  reg [AW:0] rb_w, wb_r; assign wcount = wbin - rb_w; assign rcount = {1'b0, wb_r - rbin} + valid;\n  wire [AW:0] wbin_n = wbin + 1'b1,")
f = f.replace("if (wrst) begin wbin <= 0; wgray <= 0; rg1 <= 0; rg2 <= 0; end\n    else begin rg1 <= rgray; rg2 <= rg1;", "if (wrst) begin wbin <= 0; wgray <= 0; rg1 <= 0; rg2 <= 0; rb_w <= 0; end\n    else begin rg1 <= rgray; rg2 <= rg1; rb_w <= g2b(rg2);")
f = f.replace("if (rrst) begin rbin <= 0; rgray <= 0; wg1 <= 0; wg2 <= 0; valid <= 1'b0; end\n    else begin wg1 <= wgray; wg2 <= wg1;", "if (rrst) begin rbin <= 0; rgray <= 0; wg1 <= 0; wg2 <= 0; valid <= 1'b0; wb_r <= 0; end\n    else begin wg1 <= wgray; wg2 <= wg1; wb_r <= g2b(wg2);")
assert f.count("rb_w") == 4 and f.count("wb_r") == 4, (f.count("rb_w"), f.count("wb_r")); open("ruty_fifo.v", "w").write(f)

s = open("ruty_top.v").read(); n0 = len(s)
def rep(a, b, cnt=1):
    global s; assert s.count(a) == cnt, (a[:50], s.count(a)); s = s.replace(a, b)
rep("module ruty_top #(parameter SLICES = 32) (", "module ruty_top #(parameter SLICES = 32, parameter IAW = 8) (     // input FIFO: 2^IAW words")
rep("8'd18}", "8'd19}")
rep("reg [44:0] rtot, wtot; reg [1:0] rs, ws; reg [7:0] wbeat; reg mulv; reg [3:0] gd;",
    "reg [44:0] rtot, wtot; reg [1:0] rs; reg [2:0] ws; reg [7:0] wbeat, rl, wl; reg mulv; reg [3:0] gd; reg [IAW+1:0] outst, free_r;   // rtot: words not yet requested")
rep("localparam M_IDLE = 2'd0, M_ADDR = 2'd1, M_DATA = 2'd2, M_RESP = 2'd3;", "localparam M_IDLE = 2'd0, M_ADDR = 2'd1, M_DATA = 2'd2, M_RESP = 2'd3; localparam [2:0] M_WAIT = 3'd4;\n  localparam [9:0] RCAP = 10'd64, WCAP = 10'd32;                       // longest read and write burst, in words")
a = s.index("  function [7:0] blen("); b = s.index("  wire inwin_r")
s = s[:a] + """  function [7:0] blen(input [39:0] a, input [44:0] left, input [9:0] cap); reg [9:0] room, m; begin   // beats - 1, never across a 4 KB page
    room = 10'd512 - {1'b0, a[11:3]}; m = (room < cap) ? room : cap;
    blen = (left < {35'd0, m}) ? left[7:0] - 8'd1 : m[7:0] - 8'd1; end endfunction
""" + s[b:]
rep("  wire ifull, ovalid; wire [63:0] od;", "  wire ifull, ovalid; wire [63:0] od; wire [IAW:0] icount; wire [7:0] ocount;")
rep("assign m_axi_rready = (rs == M_DATA) & (~ifull | werr);              // after a refusal: finish the burst, drop the data",
    "assign m_axi_rready = ~ifull | werr;                                 // room was reserved before the request, so this never stalls; after a refusal the data is dropped\n  wire arhs = m_axi_arvalid & m_axi_arready;")
rep("rs <= M_IDLE; ws <= M_IDLE; m_axi_arvalid <= 1'b0; m_axi_awvalid <= 1'b0; mulv <= 1'b0; gd <= 4'd0; clocks <= 32'd0; end",
    "rs <= M_IDLE; ws <= M_IDLE; m_axi_arvalid <= 1'b0; m_axi_awvalid <= 1'b0; mulv <= 1'b0; gd <= 4'd0; clocks <= 32'd0; outst <= 0; free_r <= 0; end")
a = s.index("      // read master: message words into the input FIFO"); b = s.index("      if (m_axi_awvalid & m_axi_awready) m_axi_awvalid <= 1'b0;\n      // the batch is finished")
s = s[:a] + """      // read master: a burst is requested only when the input FIFO has room for all of it, counting the words still on their way
      outst <= outst + (arhs ? {{(IAW-6){1'b0}}, rl} + 1'b1 : 1'b0) - rbeat;
      free_r <= (1 << IAW) - icount - outst;
      case (rs)
        M_ADDR: if (rtot == 45'd0 || werr) rs <= M_DATA;
                else if (~inwin_r) begin werr <= 1'b1; rs <= M_DATA; end
                else begin rl <= blen(m_axi_araddr, rtot, RCAP); rs <= M_RESP; end
        M_RESP: if (m_axi_arvalid) begin
                  if (m_axi_arready) begin m_axi_arvalid <= 1'b0; m_axi_araddr <= m_axi_araddr + {28'd0, {1'b0, rl} + 9'd1, 3'd0}; rtot <= rtot - {37'd0, rl} - 45'd1; rs <= M_ADDR; end end
                else if (werr) rs <= M_DATA;
                else if (free_r > {{(IAW-6){1'b0}}, rl}) begin m_axi_arlen <= rl; m_axi_arvalid <= 1'b1; end
        M_DATA: if (outst == 0) rs <= M_IDLE;                                                  // every requested word has arrived
        default: ;
      endcase
      // write master: a burst is requested only when all its words are waiting in the output FIFO
      case (ws)
        M_ADDR: if (wtot == 45'd0 || werr) begin ws <= M_IDLE; end
                else if (~inwin_w) begin werr <= 1'b1; ws <= M_IDLE; end
                else begin wl <= blen(m_axi_awaddr, wtot, WCAP); ws <= M_WAIT; end
        M_WAIT: if (werr) ws <= M_IDLE;
                else if (ocount > wl) begin m_axi_awlen <= wl; wbeat <= wl; m_axi_awvalid <= 1'b1; ws <= M_DATA; end
        M_DATA: begin if (m_axi_awvalid & m_axi_awready) m_axi_awvalid <= 1'b0;
                  if (wbeatgo) begin wtot <= wtot - 45'd1; m_axi_awaddr <= m_axi_awaddr + 40'd8; wbeat <= wbeat - 8'd1; if (wbeat == 8'd0) ws <= M_RESP; end end
        M_RESP: if (m_axi_bvalid & ~m_axi_awvalid) ws <= M_ADDR;
        default: ;
      endcase
""" + s[b:]
rep("ruty_fifo #(64, 6) fin(.wclk(aclk), .wrst(arst), .din(m_axi_rdata), .push(rbeat & ~werr), .full(ifull),", "ruty_fifo #(64, IAW) fin(.wclk(aclk), .wrst(arst), .din(m_axi_rdata), .push(rbeat & ~werr), .full(ifull), .wcount(icount), .rcount(),")
rep("ruty_fifo #(64, 6) fout(.wclk(eclk), .wrst(erst), .din(sd), .push(opush), .full(ofull),", "ruty_fifo #(64, 6) fout(.wclk(eclk), .wrst(erst), .din(sd), .push(opush), .full(ofull), .wcount(), .rcount(ocount),")
open("ruty_top.v", "w").write(s)
