# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty_ref.py: software model of the six functions, used by ruty_kat3.py beside the published expected values.
import hashlib
RHO = [[0,36,3,41,18],[1,44,10,45,2],[62,6,43,15,61],[28,55,25,21,56],[27,20,39,8,14]]
def rol(x, n, w): n %= w; m = (1 << w) - 1; return ((x << n) | (x >> (w - n))) & m if n else x
def rcbit(t):
    R = 1
    for _ in range(t % 255):
        R <<= 1
        if R & 0x100: R ^= 0x171
    return R & 1
def rconst(ir, w): return sum(1 << ((1 << j) - 1) for j in range(7) if (1 << j) - 1 < w and rcbit(j + 7 * ir))
def perm(A, w, first, last):
    m = (1 << w) - 1
    for ir in range(first, last + 1):
        C = [A[x] ^ A[x+5] ^ A[x+10] ^ A[x+15] ^ A[x+20] for x in range(5)]
        A = [A[i] ^ C[(i-1) % 5] ^ rol(C[(i+1) % 5], 1, w) for i in range(25)]
        B = [0] * 25
        for x in range(5):
            for y in range(5): B[y + 5*((2*x + 3*y) % 5)] = rol(A[x + 5*y], RHO[x][y], w)
        A = [B[i] ^ (~B[(i % 5 + 1) % 5 + 5*(i // 5)] & m & B[(i % 5 + 2) % 5 + 5*(i // 5)]) for i in range(25)]
        A[0] ^= rconst(ir, w)
    return A
def ref_xof(msg, outlen, w, rate, first, last, D=0x1F):
    nb = w // 8; p = bytearray(msg) + bytes([D]); p += bytes(-len(p) % rate); p[-1] ^= 0x80; A = [0] * 25; out = b""
    for b in range(0, len(p), rate):
        for i in range(rate // nb): A[i] ^= int.from_bytes(p[b + nb*i: b + nb*i + nb], "little")
        A = perm(A, w, first, last)
    while True:
        out += b"".join(a.to_bytes(nb, "little") for a in A[:rate // nb])
        if len(out) >= outlen: return out[:outlen]
        A = perm(A, w, first, last)
#        name            lane bits, rate bytes, first, last
ALG = {"shake128": (64, 168, 0, 23), "shake256": (64, 136, 0, 23), "turboshake128": (64, 168, 12, 23), "turboshake256": (64, 136, 12, 23),
       "minishake128": (32, 68, 10, 21), "minishake256": (32, 36, 10, 21)}
def soft(name, msg, outlen):
    if name == "shake128": return hashlib.shake_128(msg).digest(outlen)
    if name == "shake256": return hashlib.shake_256(msg).digest(outlen)
    w, rate, f, l = ALG[name]; return ref_xof(msg, outlen, w, rate, f, l)
