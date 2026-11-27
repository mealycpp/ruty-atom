# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Probe for chain mode: single cases with status, clock count and the debug registers printed.
import sys
src = open("ruty_chain.py").read(); exec(src[:src.index("bad = 0")])
import random
random.seed(9); rb = lambda n: bytes(random.getrandbits(8) for _ in range(n))
def tpl(start): m = bytearray(rb(64)); m[47] = start; return bytes(m)
def case(name, n, start, cend, dom=None):
    if dom is not None: DOM[name] = dom
    tp = [tpl(start) for _ in range(n)]; d, ck, st = chains(name, tp, cend); exp = [model(name, t, cend) for t in tp]
    good = sum(a == b for a, b in zip(d, exp))
    print("RESULT dbg alg=%s chains=%d addr=%d..%d dom=%02x status=%02x clocks=%d correct=%d/%d dbg0=%08x dbg1=%08x got0=%s exp0=%s" % (
        name, n, start, cend - 1, DOM.get(name, 0x1F), st, ck, good, n, reg.read(0x3C), reg.read(0x40), d[0].hex()[:16], exp[0].hex()[:16]), flush=True)
    if st & 1:
        print("RESULT dbg engine busy: reloading the image"); globals()["ol"] = Overlay(bit); reg.write(0x28, lo); reg.write(0x2C, hi)
case('turboshake256', 2, 0, 15)
case('minishake128', 2, 0, 15, 0x01)
case('turboshake256', 36, 0, 15)
case('minishake128', 36, 0, 15, 0x01)
case('turboshake256', 2, 0, 15)
case('turboshake256', 4, 0, 1)
case('turboshake256', 36, 7, 15)
case('turboshake256', 4, 14, 15)
case('turboshake256', 2, 0, 15)
case('minishake128', 36, 0, 15, 0x01)
case('minishake128', 4, 0, 1, 0x01)
case('shake256', 2, 0, 15)
