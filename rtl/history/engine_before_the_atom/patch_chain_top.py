# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""v23: registers for chain mode (0x44: bit 0 on, [12:8] counter lane, [18:16] counter byte, [24:20] feedback lane; 0x48: end value)."""
t = open("ruty_top.v").read()
def rt(a, b):
    global t; assert t.count(a) == 1, a[:60]; t = t.replace(a, b)
rt("8'd22}", "8'd23}")
rt("          6'd12: if (s_axi_wdata[0]) wlock <= 1'b1;", "          6'd12: if (s_axi_wdata[0]) wlock <= 1'b1;\n          6'd17: if (~busy) begin iter <= s_axi_wdata[0]; cw <= s_axi_wdata[12:8]; cb <= s_axi_wdata[18:16]; fbw <= s_axi_wdata[24:20]; end\n          6'd18: if (~busy) cend <= s_axi_wdata[7:0];")
rt("reg wlock, busy, done, werr, go_t;", "reg wlock, busy, done, werr, go_t; reg iter; reg [4:0] cw, fbw; reg [2:0] cb; reg [7:0] cend;")
rt("wlock <= 1'b0; wbase <= 28'd0;", "wlock <= 1'b0; iter <= 1'b0; cw <= 5'd0; cb <= 3'd0; fbw <= 5'd0; cend <= 8'd0; wbase <= 28'd0;")
rt("          6'd15: s_axi_rdata <= dbg0; 6'd16: s_axi_rdata <= dbg1;", "          6'd15: s_axi_rdata <= dbg0; 6'd16: s_axi_rdata <= dbg1;\n          6'd17: s_axi_rdata <= {7'd0, fbw, 1'b0, cb, 3'd0, cw, 7'd0, iter}; 6'd18: s_axi_rdata <= {24'd0, cend};")
rt(".dual(dual), .rate(rate),", ".dual(dual), .rate(rate), .iter(iter), .cw(cw), .cb(cb), .fbw(fbw), .cend(cend),")
open("ruty_top.v", "w").write(t)
