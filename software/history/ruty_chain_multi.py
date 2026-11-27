# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# RUTY chain mode with N engines on one memory port: every engine runs its own batch of chains at the same time.
# Time per engine comes from its hardware counter (bus clocks from its go to its last digest in memory).
# usage: python3 ruty_chain_multi.py ruty.bit N [engine MHz] [bus MHz]
import sys, time, hashlib, random, numpy as np
from pynq import Overlay, MMIO, allocate
from ruty_ref import ALG, ref_xof
bit = sys.argv[1]; N = int(sys.argv[2]); ENG = float(sys.argv[3]) if len(sys.argv) > 3 else 0; BUS = float(sys.argv[4]) if len(sys.argv) > 4 else 300.0
ol = Overlay(bit); regs = [MMIO(0xA0000000 + 0x1000 * i, 0x1000) for i in range(N)]
print("RESULT engines=%d engine_mhz=%g ids=%s" % (N, ENG, " ".join("%08x" % r.read(0x38) for r in regs)))
NW = 1 << 17; bufs = [(allocate(shape=(NW,), dtype=np.uint64), allocate(shape=(NW,), dtype=np.uint64)) for _ in range(N)]
for r, (a, b) in zip(regs, bufs):
    lo = min(a.device_address, b.device_address) >> 12; hi = (max(a.device_address, b.device_address) + 8 * NW + 4095) >> 12
    r.write(0x28, lo); r.write(0x2C, hi)
DOM = {"minishake128": 0x01}
def model(name, tpl, cend, olen=16):
    w, rate, f, l = ALG[name]; m = bytearray(tpl); D = DOM.get(name, 0x1F)
    while True:
        d = hashlib.shake_256(bytes(m)).digest(olen) if name == "shake256" else ref_xof(bytes(m), olen, w, rate, f, l, D)
        m[48:48 + olen] = d; m[47] = (m[47] + 1) & 255
        if m[47] == cend: return d
random.seed(9); rb = lambda n: bytes(random.getrandbits(8) for _ in range(n)); bad = 0
def tpl(): m = bytearray(rb(64)); m[47] = 0; return bytes(m)
n = 8000
for name in ("shake256", "turboshake256", "minishake128"):
    w, rate, f, l = ALG[name]; dual = w == 32; nb = w // 8; wpm = 64 // nb; wpo = 16 // nb; nj = n // 2 if dual else n; sets = []
    for i in range(N):
        base = [tpl() for _ in range(16)]; tp = (base * (n // 16 + 1))[:n]; sets.append(base)
        raw = np.frombuffer(b"".join(tp), dtype=np.uint8).reshape(n, 64)
        words = np.ascontiguousarray(raw.reshape(nj, 2, wpm, 4).transpose(0, 2, 1, 3)).reshape(-1).view(np.uint64) if dual else np.ascontiguousarray(raw).reshape(-1).view(np.uint64)
        a, b = bufs[i]; a[:len(words)] = words; a.flush(); r = regs[i]
        r.write(0x08, a.device_address & 0xffffffff); r.write(0x0C, a.device_address >> 32); r.write(0x10, b.device_address & 0xffffffff); r.write(0x14, b.device_address >> 32)
        r.write(0x18, nj); r.write(0x1C, 64 | (DOM.get(name, 0x1F) << 24)); r.write(0x20, 16); r.write(0x24, f | (l << 8) | (dual << 16) | ((rate // nb) << 24))
        r.write(0x44, 1 | ((47 // nb) << 8) | ((47 % nb) << 16) | ((48 // nb) << 20)); r.write(0x48, 15)
    for k in sorted(set((1, N))):
        for i in range(k): bufs[i][1][:nj * wpo] = 0; bufs[i][1].flush(); regs[i].write(0x00, 2)
        t0 = time.perf_counter()
        for i in range(k): regs[i].write(0x00, 1)
        while any(regs[i].read(0x04) & 1 for i in range(k)):
            if time.perf_counter() - t0 > 30: break
        wall = time.perf_counter() - t0; per = []; f_ = 0; rate_ = 0.0
        for i in range(k):
            st = regs[i].read(0x04); ck = regs[i].read(0x34); per.append(ck * 1000.0 / BUS / (15 * n)); rate_ += n / (ck / (BUS * 1e6)); bufs[i][1].invalidate()
            o = np.array(bufs[i][1][:nj * wpo]).view(np.uint8)
            d = np.ascontiguousarray(o.reshape(nj, wpo, 2, 4).transpose(0, 2, 1, 3)).reshape(n, 16) if dual else o.reshape(n, 16)
            ref = [model(name, t, 15) for t in sets[i]]
            f_ += sum(bytes(d[j]) != ref[j % 16] for j in range(n)) + (st != 2) * n
        bad += f_
        print("RESULT chain_multi alg=%s running=%d chains_each=%d ns_per_step=%.1f..%.1f Mchains_per_s=%.3f wall_ms=%.1f mismatches=%d" % (name, k, n, min(per), max(per), rate_ / 1e6, wall * 1e3, f_), flush=True)
    for r in regs: r.write(0x44, 0)
print("RESULT chain_multi summary mismatches=%d %s" % (bad, "FAIL" if bad else "ALL_MATCH"))
