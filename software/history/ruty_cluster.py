# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# RUTY dynamic cluster on one bitstream: k of the N atoms run chains at once, j of them split (MiniSHAKE128) and k-j wide (SHAKE256).
# The cluster is re-formed between runs by register writes only. Throughput is taken from the hardware counter of every atom.
# usage: python3 ruty_cluster.py ruty.bit N [engine MHz] [bus MHz]      prints one RESULT line per (k, j)
import sys, time, hashlib, random, numpy as np
from pynq import Overlay, MMIO, allocate
from ruty_ref import ALG, ref_xof
bit = sys.argv[1]; N = int(sys.argv[2]); ENG = float(sys.argv[3]) if len(sys.argv) > 3 else 0; BUS = float(sys.argv[4]) if len(sys.argv) > 4 else 300.0
ol = Overlay(bit); regs = [MMIO(0xA0000000 + 0x1000 * i, 0x1000) for i in range(N)]
print("RESULT cluster engines=%d engine_mhz=%g ids=%s" % (N, ENG, " ".join("%08x" % r.read(0x38) for r in regs)))
n = 4000; NW = 1 << 16
bufs = [dict(shake256=allocate(shape=(NW,), dtype=np.uint64), minishake128=allocate(shape=(NW,), dtype=np.uint64), out=allocate(shape=(NW,), dtype=np.uint64)) for _ in range(N)]
DOM = {"minishake128": 0x01}
def model(name, tpl, cend, olen=16):
    w, rate, f, l = ALG[name]; m = bytearray(tpl); D = DOM.get(name, 0x1F)
    while True:
        d = hashlib.shake_256(bytes(m)).digest(olen) if name == "shake256" else ref_xof(bytes(m), olen, w, rate, f, l, D)
        m[48:48 + olen] = d; m[47] = (m[47] + 1) & 255
        if m[47] == cend: return d
random.seed(11); rb = lambda k: bytes(random.getrandbits(8) for _ in range(k)); ref = {}
def tpl(): m = bytearray(rb(64)); m[47] = 0; return bytes(m)
for i in range(N):
    ads = [bufs[i][k].device_address for k in bufs[i]]; regs[i].write(0x28, min(ads) >> 12); regs[i].write(0x2C, (max(ads) + 8 * NW + 4095) >> 12)
    for name in ("shake256", "minishake128"):
        w, rate, f, l = ALG[name]; dual = w == 32; base = [tpl() for _ in range(16)]; tp = (base * (n // 16 + 1))[:n]
        raw = np.frombuffer(b"".join(tp), dtype=np.uint8).reshape(n, 64)
        words = np.ascontiguousarray(raw.reshape(n // 2, 2, 16, 4).transpose(0, 2, 1, 3)).reshape(-1).view(np.uint64) if dual else np.ascontiguousarray(raw).reshape(-1).view(np.uint64)
        bufs[i][name][:len(words)] = words; bufs[i][name].flush(); ref[(i, name)] = [model(name, t, 15) for t in base]
def arm(i, name):
    w, rate, f, l = ALG[name]; dual = w == 32; nb = w // 8; nj = n // 2 if dual else n; a = bufs[i][name]; b = bufs[i]["out"]; r = regs[i]
    r.write(0x08, a.device_address & 0xffffffff); r.write(0x0C, a.device_address >> 32); r.write(0x10, b.device_address & 0xffffffff); r.write(0x14, b.device_address >> 32)
    r.write(0x18, nj); r.write(0x1C, 64 | (DOM.get(name, 0x1F) << 24)); r.write(0x20, 16); r.write(0x24, f | (l << 8) | (dual << 16) | ((rate // nb) << 24))
    r.write(0x44, 1 | ((47 // nb) << 8) | ((47 % nb) << 16) | ((48 // nb) << 20)); r.write(0x48, 15); b[:32] = 0; b.flush(); r.write(0x00, 2)
bad = 0
for k in range(1, N + 1):
    for j in range(0, k + 1):
        names = ["minishake128"] * j + ["shake256"] * (k - j)
        for i in range(k): arm(i, names[i])
        t0 = time.perf_counter()
        for i in range(k): regs[i].write(0x00, 1)
        while any(regs[i].read(0x04) & 1 for i in range(k)):
            if time.perf_counter() - t0 > 20: break
        rate_ = 0.0; f_ = 0
        for i in range(k):
            st = regs[i].read(0x04); ck = regs[i].read(0x34); rate_ += n / (ck / (BUS * 1e6)); bufs[i]["out"].invalidate(); o = np.array(bufs[i]["out"][:32]).view(np.uint8)
            d = np.ascontiguousarray(o.reshape(8, 4, 2, 4).transpose(0, 2, 1, 3)).reshape(16, 16) if names[i] == "minishake128" else o.reshape(16, 16)
            f_ += sum(bytes(d[q]) != ref[(i, names[i])][q] for q in range(16)) + (st != 2) * 16
        bad += f_
        print("RESULT cluster atoms=%d split=%d wide=%d Mchains_per_s=%.3f mismatches=%d" % (k, j, k - j, rate_ / 1e6, f_), flush=True)
for r in regs: r.write(0x44, 0)
print("RESULT cluster summary mismatches=%d %s" % (bad, "FAIL" if bad else "ALL_MATCH"))
