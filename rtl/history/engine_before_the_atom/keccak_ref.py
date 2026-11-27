# SPDX-License-Identifier: CERN-OHL-S-2.0
# Copyright (c) 2026 Mohamed El-Hadedy
RHO64 = [[0,36,3,41,18],[1,44,10,45,2],[62,6,43,15,61],[28,55,25,21,56],[27,20,39,8,14]]   # RHO64[x][y]
def _rc_bit(t):
    R = 1
    for _ in range(t % 255):
        R <<= 1
        if R & 0x100: R ^= 0x171
    return R & 1
def round_constant(ir, w=64):
    rc = 0
    for j in range(7):
        pos = (1 << j) - 1
        if pos < w and _rc_bit(j + 7 * ir): rc |= 1 << pos
    return rc
