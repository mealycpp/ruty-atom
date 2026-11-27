// SPDX-License-Identifier: CERN-OHL-S-2.0
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_axil: AXI4-Lite register front end for the RUTY engine (bring-up version: the processor drives every step).
//  0x000 CTRL    W  bit0 start core 0, bit1 start core 1 (pulses)
//  0x004 MODE    RW bit0 dual: 0 = one Keccak-p[1600] core, 1 = two Keccak-p[800] cores
//  0x008 ROUNDS0 RW [4:0] first round, [12:8] last round (core 0, or the one core)
//  0x00C ROUNDS1 RW same for core 1
//  0x010 STATUS  R  [1:0] busy, [3:2] request waiting
//  0x014 CLOCKS0 R  engine clocks from start to finish of the last core-0 job (counted in hardware)
//  0x018 CLOCKS1 R  same for core 1
//  0x01C ID      R  0x5255 in the top half, slices per clock in [15:8], version in [7:0]
//  0x100 + 8*i   RW lane i, low 32 bits (A half);  0x104 + 8*i  high 32 bits (B half)
//  0x200 + 8*i   W  xor into lane i, low half;      0x204 + 8*i  high half
module ruty_axil #(parameter SLICES = 16) (
    input aclk, input aresetn,
    input [11:0] s_axi_awaddr, input s_axi_awvalid, output s_axi_awready,
    input [31:0] s_axi_wdata, input [3:0] s_axi_wstrb, input s_axi_wvalid, output s_axi_wready,
    output [1:0] s_axi_bresp, output reg s_axi_bvalid, input s_axi_bready,
    input [11:0] s_axi_araddr, input s_axi_arvalid, output s_axi_arready,
    output reg [31:0] s_axi_rdata, output [1:0] s_axi_rresp, output reg s_axi_rvalid, input s_axi_rready);
  wire rst = ~aresetn;
  reg dual; reg [4:0] f0, l0, f1, l1; reg [1:0] start, wr; reg wxor; reg [4:0] widx, ridx; reg [63:0] wdata;
  wire [63:0] lane; wire [1:0] busy, qfull;
  ruty_core core(.clk(aclk), .rst(rst), .dual(dual), .wr(wr), .wxor(wxor), .widx(widx), .wdata(wdata), .ridx(ridx), .rdata(lane),
                 .start(start), .first0(f0), .last0(l0), .first1(f1), .last1(l1), .busy(busy), .qfull(qfull));
  assign s_axi_bresp = 2'b00; assign s_axi_rresp = 2'b00;
  wire wfire = s_axi_awvalid & s_axi_wvalid & ~s_axi_bvalid;
  assign s_axi_awready = wfire; assign s_axi_wready = wfire;
  wire [11:0] wa = s_axi_awaddr;
  reg [1:0] rs; reg [11:0] ra; reg [31:0] clk0, clk1;
  assign s_axi_arready = s_axi_arvalid & (rs == 2'd0) & ~s_axi_rvalid;
  always @(posedge aclk) begin
    start <= 2'b00; wr <= 2'b00;
    if (rst) begin s_axi_bvalid <= 1'b0; s_axi_rvalid <= 1'b0; rs <= 2'd0; dual <= 1'b0; f0 <= 5'd0; l0 <= 5'd23; f1 <= 5'd0; l1 <= 5'd23;
      clk0 <= 32'd0; clk1 <= 32'd0; end
    else begin
      if (wfire) begin
        s_axi_bvalid <= 1'b1;
        case (wa[11:8])
          4'h0: case (wa[7:2])
                  6'd0: start <= s_axi_wdata[1:0];
                  6'd1: dual <= s_axi_wdata[0];
                  6'd2: begin f0 <= s_axi_wdata[4:0]; l0 <= s_axi_wdata[12:8]; end
                  6'd3: begin f1 <= s_axi_wdata[4:0]; l1 <= s_axi_wdata[12:8]; end
                  default: ;
                endcase
          4'h1, 4'h2: begin wr <= wa[2] ? 2'b10 : 2'b01; widx <= wa[7:3]; wdata <= {s_axi_wdata, s_axi_wdata}; wxor <= wa[9]; end
          default: ;
        endcase
      end else if (s_axi_bvalid & s_axi_bready) s_axi_bvalid <= 1'b0;
      // read: latch the address, give the lane read two clocks, then answer
      if (s_axi_arready) begin ra <= s_axi_araddr; ridx <= s_axi_araddr[7:3]; rs <= 2'd1; end
      else if (rs != 2'd0) begin
        rs <= rs + 2'd1;
        if (rs == 2'd3) begin
          s_axi_rvalid <= 1'b1;
          if (ra[11:8] == 4'h1) s_axi_rdata <= ra[2] ? lane[63:32] : lane[31:0];
          else case (ra[7:2])
            6'd1: s_axi_rdata <= {31'd0, dual};
            6'd2: s_axi_rdata <= {19'd0, l0, 3'd0, f0};
            6'd3: s_axi_rdata <= {19'd0, l1, 3'd0, f1};
            6'd4: s_axi_rdata <= {28'd0, qfull, busy};
            6'd5: s_axi_rdata <= clk0;
            6'd6: s_axi_rdata <= clk1;
            6'd7: s_axi_rdata <= {16'h5255, SLICES[7:0], 8'd2};
            default: s_axi_rdata <= 32'd0;
          endcase
        end
      end
      if (s_axi_rvalid & s_axi_rready) s_axi_rvalid <= 1'b0;
      if (start[0]) clk0 <= 32'd1; else if (busy[0]) clk0 <= clk0 + 32'd1;
      if (start[1]) clk1 <= 32'd1; else if (busy[1]) clk1 <= clk1 + 32'd1;
    end
  end
endmodule
