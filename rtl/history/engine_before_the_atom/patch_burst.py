# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
"""ruty_top.v: bursts of up to 256 beats instead of 16 (the board showed the 16-beat bursts starve the engine on long messages)."""
import os; os.chdir(os.path.dirname(os.path.abspath(__file__)))
s = open("ruty_top.v").read()
a = "blen = (left < 45'd16 && left <= {35'd0, room}) ? left[7:0] - 8'd1 : (room < 10'd16) ? room[7:0] - 8'd1 : 8'd15;"
assert s.count(a) == 1
s = s.replace(a, "blen = (left < 45'd256 && left <= {35'd0, room}) ? left[7:0] - 8'd1 : (room < 10'd256) ? room[7:0] - 8'd1 : 8'd255;")
assert s.count("8'd17};") == 1; s = s.replace("8'd17};", "8'd18};")
open("ruty_top.v", "w").write(s)
