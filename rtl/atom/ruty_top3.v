// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_top: RUTY as a bus master. The processor writes a batch description into a few registers and pulses go.
// RUTY then reads every message from memory itself, hashes the batch, and writes every digest back to memory.
// Two clocks: aclk (both AXI interfaces) and eclk (engine and sponge sequencer), joined by two FIFOs.
//  0x00 CTRL   W  bit0 go, bit1 clear done/error flags
//  0x04 STATUS R  bit0 busy, bit1 done, [5:4] engine refused a request, bit6 address outside the window
//  0x08 IN_LO  0x0C IN_HI    address of the first message word     0x10 OUT_LO  0x14 OUT_HI  address of the first digest word
//  0x18 NJOBS  jobs in the batch (dual: pairs)
//  0x1C LEN    [23:0] message bytes per job, [31:24] domain byte
//  0x20 OLEN   [15:0] digest bytes per job
//  0x24 ALG    [4:0] first round, [12:8] last round, [16] dual (two 800-bit cores), [28:24] rate in lanes
//  0x28 WBASE  0x2C WLIMIT   the master may only touch 4 KB pages  WBASE <= page < WLIMIT
//  0x30 WLOCK  bit0: once set, WBASE and WLIMIT cannot change until reset
//  0x34 CLOCKS R  aclk clocks from go to done      0x38 ID R
// Memory layout: each job owns ceil(LEN/8) input words and ceil(OLEN/8) output words (dual: ceil(/4), one word = 4 bytes
// of each of the two jobs, core 0 in the low half). Jobs are packed back to back.
module ruty_top #(parameter SLICES = 32, parameter IAW = 8) (     // input FIFO: 2^IAW words
    (* X_INTERFACE_INFO = "xilinx.com:signal:clock:1.0 aclk CLK", X_INTERFACE_PARAMETER = "ASSOCIATED_BUSIF s_axi:m_axi, ASSOCIATED_RESET aresetn" *) input aclk,
    input aresetn,
    (* X_INTERFACE_INFO = "xilinx.com:signal:clock:1.0 eclk CLK" *) input eclk,
    input [11:0] s_axi_awaddr, input s_axi_awvalid, output s_axi_awready,
    input [31:0] s_axi_wdata, input [3:0] s_axi_wstrb, input s_axi_wvalid, output s_axi_wready,
    output [1:0] s_axi_bresp, output reg s_axi_bvalid, input s_axi_bready,
    input [11:0] s_axi_araddr, input s_axi_arvalid, output s_axi_arready,
    output reg [31:0] s_axi_rdata, output [1:0] s_axi_rresp, output reg s_axi_rvalid, input s_axi_rready,
    output reg [39:0] m_axi_araddr, output reg [7:0] m_axi_arlen, output [2:0] m_axi_arsize, output [1:0] m_axi_arburst, output reg m_axi_arvalid, input m_axi_arready,
    input [63:0] m_axi_rdata, input [1:0] m_axi_rresp, input m_axi_rlast, input m_axi_rvalid, output m_axi_rready,
    output reg [39:0] m_axi_awaddr, output reg [7:0] m_axi_awlen, output [2:0] m_axi_awsize, output [1:0] m_axi_awburst, output reg m_axi_awvalid, input m_axi_awready,
    output [63:0] m_axi_wdata, output [7:0] m_axi_wstrb, output m_axi_wlast, output m_axi_wvalid, input m_axi_wready,
    input [1:0] m_axi_bresp, input m_axi_bvalid, output m_axi_bready);
  wire rst = ~aresetn;
  wire [31:0] dbg0, dbg1;
  // ---------------- registers (aclk) ----------------
  reg [39:0] in_a, out_a; reg [23:0] njobs, len; reg [7:0] dom; reg [15:0] olen; reg [4:0] first, last, rate; reg dual;
  reg [27:0] wbase, wlimit; reg wlock, busy, done, werr, go_t; reg iter; reg [4:0] cw, fbw; reg [2:0] cb; reg [7:0] cend; reg [31:0] clocks; reg go, clrf;
  assign s_axi_bresp = 2'b00; assign s_axi_rresp = 2'b00;
  wire wfire = s_axi_awvalid & s_axi_wvalid & ~s_axi_bvalid;
  assign s_axi_awready = wfire; assign s_axi_wready = wfire;
  assign s_axi_arready = s_axi_arvalid & ~s_axi_rvalid;
  (* ASYNC_REG = "TRUE" *) reg [1:0] ee1, ee2; (* ASYNC_REG = "TRUE" *) reg si1, si2; wire [1:0] eerr; wire seq_idle;
  always @(posedge aclk) begin
    go <= 1'b0; clrf <= 1'b0; ee1 <= eerr; ee2 <= ee1; si1 <= seq_idle; si2 <= si1;
    if (rst) begin s_axi_bvalid <= 1'b0; s_axi_rvalid <= 1'b0; wlock <= 1'b0; iter <= 1'b0; cw <= 5'd0; cb <= 3'd0; fbw <= 5'd0; cend <= 8'd0; wbase <= 28'd0; wlimit <= 28'd0; njobs <= 24'd0; dual <= 1'b0;
      first <= 5'd0; last <= 5'd23; rate <= 5'd21; len <= 24'd0; olen <= 16'd32; dom <= 8'h1f; in_a <= 40'd0; out_a <= 40'd0; end
    else begin
      if (wfire) begin
        s_axi_bvalid <= 1'b1;
        case (s_axi_awaddr[7:2])
          6'd0: begin go <= s_axi_wdata[0] & ~busy; clrf <= s_axi_wdata[1]; end
          6'd2: if (~busy) in_a[31:0] <= s_axi_wdata;       6'd3: if (~busy) in_a[39:32] <= s_axi_wdata[7:0];
          6'd4: if (~busy) out_a[31:0] <= s_axi_wdata;      6'd5: if (~busy) out_a[39:32] <= s_axi_wdata[7:0];
          6'd6: if (~busy) njobs <= s_axi_wdata[23:0];
          6'd7: if (~busy) begin len <= s_axi_wdata[23:0]; dom <= s_axi_wdata[31:24]; end
          6'd8: if (~busy) olen <= s_axi_wdata[15:0];
          6'd9: if (~busy) begin first <= s_axi_wdata[4:0]; last <= s_axi_wdata[12:8]; dual <= s_axi_wdata[16]; rate <= s_axi_wdata[28:24]; end
          6'd10: if (~wlock) wbase <= s_axi_wdata[27:0];    6'd11: if (~wlock) wlimit <= s_axi_wdata[27:0];
          6'd12: if (s_axi_wdata[0]) wlock <= 1'b1;
          6'd17: if (~busy) begin iter <= s_axi_wdata[0]; cw <= s_axi_wdata[12:8]; cb <= s_axi_wdata[18:16]; fbw <= s_axi_wdata[24:20]; end
          6'd18: if (~busy) cend <= s_axi_wdata[7:0];
          default: ;
        endcase
      end else if (s_axi_bvalid & s_axi_bready) s_axi_bvalid <= 1'b0;
      if (s_axi_arready) begin
        s_axi_rvalid <= 1'b1;
        case (s_axi_araddr[7:2])
          6'd1: s_axi_rdata <= {25'd0, werr, ee2, 2'b00, done, busy};
          6'd2: s_axi_rdata <= in_a[31:0];   6'd3: s_axi_rdata <= {24'd0, in_a[39:32]};
          6'd4: s_axi_rdata <= out_a[31:0];  6'd5: s_axi_rdata <= {24'd0, out_a[39:32]};
          6'd6: s_axi_rdata <= {8'd0, njobs}; 6'd7: s_axi_rdata <= {dom, len}; 6'd8: s_axi_rdata <= {16'd0, olen};
          6'd9: s_axi_rdata <= {3'd0, rate, 7'd0, dual, 3'd0, last, 3'd0, first};
          6'd10: s_axi_rdata <= {4'd0, wbase}; 6'd11: s_axi_rdata <= {4'd0, wlimit}; 6'd12: s_axi_rdata <= {31'd0, wlock};
          6'd13: s_axi_rdata <= clocks;
          6'd14: s_axi_rdata <= {16'h5255, SLICES[7:0], 8'd33};
          6'd15: s_axi_rdata <= dbg0; 6'd16: s_axi_rdata <= dbg1;
          6'd17: s_axi_rdata <= {7'd0, fbw, 1'b0, cb, 3'd0, cw, 7'd0, iter}; 6'd18: s_axi_rdata <= {24'd0, cend};
          default: s_axi_rdata <= 32'd0;
        endcase
      end else if (s_axi_rvalid & s_axi_rready) s_axi_rvalid <= 1'b0;
    end
  end
  // ---------------- batch control and the two bus masters (aclk) ----------------
  wire [20:0] wpm = dual ? ((len + 24'd3) >> 2) : ((len + 24'd7) >> 3);          // words per message
  wire [13:0] wpo = dual ? ((olen + 16'd3) >> 2) : ((olen + 16'd7) >> 3);        // words per digest
  reg [44:0] rtot, wtot; reg [1:0] rs; reg [2:0] ws; reg [7:0] wbeat, rl, wl; reg mulv; reg [3:0] gd; reg [IAW+1:0] outst, free_r;   // rtot: words not yet requested
  reg [23:0] nj_r; reg [20:0] wpm_r; reg [13:0] wpo_r; reg [44:0] pr1, pr2, pw1, pw2;       // product pipeline
  always @(posedge aclk) begin nj_r <= njobs; wpm_r <= wpm; wpo_r <= wpo; pr1 <= nj_r * wpm_r; pr2 <= pr1; pw1 <= nj_r * wpo_r; pw2 <= pw1; end
  localparam M_IDLE = 2'd0, M_ADDR = 2'd1, M_DATA = 2'd2, M_RESP = 2'd3; localparam [2:0] M_WAIT = 3'd4;
  localparam [9:0] RCAP = 10'd64, WCAP = 10'd32;                       // longest read and write burst, in words
  assign m_axi_arsize = 3'd3; assign m_axi_arburst = 2'b01; assign m_axi_awsize = 3'd3; assign m_axi_awburst = 2'b01; assign m_axi_wstrb = 8'hff;
  function [7:0] blen(input [39:0] a, input [44:0] left, input [9:0] cap); reg [9:0] room, m; begin   // beats - 1, never across a 4 KB page
    room = 10'd512 - {1'b0, a[11:3]}; m = (room < cap) ? room : cap;
    blen = (left < {35'd0, m}) ? left[7:0] - 8'd1 : m[7:0] - 8'd1; end endfunction
  wire inwin_r = (m_axi_araddr[39:12] >= wbase) & (m_axi_araddr[39:12] < wlimit);
  wire inwin_w = (m_axi_awaddr[39:12] >= wbase) & (m_axi_awaddr[39:12] < wlimit);
  wire ifull, ovalid; wire [63:0] od; wire [IAW:0] icount; wire [7:0] ocount;
  assign m_axi_rready = ~ifull | werr;                                 // room was reserved before the request, so this never stalls; after a refusal the data is dropped
  wire arhs = m_axi_arvalid & m_axi_arready;
  assign dbg0 = {ws, rs, si2, ovalid, ifull, ocount, mulv, go_t, m_axi_awvalid, m_axi_arvalid, wl, 4'd0};
  reg [5:0] wcnt; reg [3:0] bcnt, awcnt;
  always @(posedge aclk) if (go) begin wcnt <= 0; bcnt <= 0; awcnt <= 0; end else begin wcnt <= wcnt + wbeatgo; bcnt <= bcnt + (m_axi_bvalid & m_axi_bready); awcnt <= awcnt + (m_axi_awvalid & m_axi_awready); end
  assign dbg1 = {wtot[7:0], m_axi_bvalid, m_axi_wready, m_axi_awready, m_axi_wlast, awcnt, bcnt, wcnt, wbeat[5:0]};
  wire rbeat = m_axi_rvalid & m_axi_rready;
  assign m_axi_wvalid = (ws == M_DATA) & ~m_axi_awvalid & (ovalid | werr); assign m_axi_wdata = od; assign m_axi_wlast = (wbeat == 8'd0);
  wire wbeatgo = m_axi_wvalid & m_axi_wready;
  assign m_axi_bready = (ws == M_RESP);
  always @(posedge aclk) begin
    if (rst) begin busy <= 1'b0; done <= 1'b0; werr <= 1'b0; go_t <= 1'b0; rs <= M_IDLE; ws <= M_IDLE; m_axi_arvalid <= 1'b0; m_axi_awvalid <= 1'b0; mulv <= 1'b0; gd <= 4'd0; clocks <= 32'd0; outst <= 0; free_r <= 0; end
    else begin
      if (clrf & ~busy) begin done <= 1'b0; werr <= 1'b0; end
      gd <= {gd[2:0], go}; mulv <= gd[3];                                                      // wait for the products
      if (go) begin busy <= 1'b1; done <= 1'b0; clocks <= 32'd0; m_axi_araddr <= in_a; m_axi_awaddr <= out_a; end
      else if (busy) clocks <= clocks + 32'd1;
      if (mulv) begin rtot <= pr2; wtot <= pw2; go_t <= ~go_t; rs <= M_ADDR; ws <= M_ADDR; end                           // products are ready: release both masters
      // read master: a burst is requested only when the input FIFO has room for all of it, counting the words still on their way
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
      if (m_axi_awvalid & m_axi_awready) m_axi_awvalid <= 1'b0;
      // the batch is finished when every digest word is in memory (or an address was refused)
      if (busy & ~mulv & ~go & (gd == 4'd0) & (rs == M_IDLE) & (ws == M_IDLE) & (si2 | werr)) begin busy <= 1'b0; done <= 1'b1; end
    end
  end
  // ---------------- engine clock domain ----------------
  (* ASYNC_REG = "TRUE" *) reg [2:0] er; (* ASYNC_REG = "TRUE" *) reg [2:0] gs;
  wire arst = rst | werr;                                              // a refused address also resets the engine side
  always @(posedge eclk) begin er <= {er[1:0], arst}; gs <= {gs[1:0], go_t}; end
  wire erst = er[2], ego = gs[2] ^ gs[1];
  wire [63:0] id, sd; wire ivalid, iready, opush, ofull;
  ruty_fifo #(64, IAW) fin(.wclk(aclk), .wrst(arst), .din(m_axi_rdata), .push(rbeat & ~werr), .full(ifull), .wcount(icount), .rcount(), .rclk(eclk), .rrst(erst), .dout(id), .valid(ivalid), .ready(iready));
  ruty_fifo #(64, 6) fout(.wclk(eclk), .wrst(erst), .din(sd), .push(opush), .full(ofull), .wcount(), .rcount(ocount), .rclk(aclk), .rrst(arst), .dout(od), .valid(ovalid), .ready(wbeatgo));
  ruty_seq seq(.clk(eclk), .rst(erst), .go(ego), .njobs(njobs), .len(len), .dom(dom), .olen(olen), .first(first), .last(last), .dual(dual), .rate(rate), .iter(iter), .cw(cw), .cb(cb), .fbw(fbw), .cend(cend),
               .idata(id), .ivalid(ivalid), .iready(iready), .odata(sd), .opush(opush), .ofull(ofull), .idle(seq_idle), .eerr(eerr));
endmodule
