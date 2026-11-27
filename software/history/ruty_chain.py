# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# RUTY chain mode on the board: Winternitz chains of hash-based signatures run inside the engine.
# A job is a 64-byte template: seed (16) || address (32, hash address in its last byte) || value (16). The engine hashes it,
# writes the 16-byte digest back as the value, counts the hash address up, and repeats until the address equals CEND.
# SHAKE256 with this template is the chain of SLH-DSA-SHAKE-128s/f (FIPS 205); the fixed vector below was produced with an
# independent implementation (PyPI slh-dsa, WOTS chain). TurboSHAKE256 and MiniSHAKE128 use the same template: experimental
# variants, not FIPS 205. MiniSHAKE128 runs two chains at once (two-core mode), domain byte 0x01 as in its specification.
# usage: python3 ruty_chain.py ruty.bit [bus MHz]
import sys, time, hashlib, random, numpy as np
from pynq import Overlay, MMIO, allocate
from ruty_ref import ALG, ref_xof
bit = sys.argv[1]; BUS = float(sys.argv[2]) if len(sys.argv) > 2 else 300.0
ol = Overlay(bit); reg = MMIO(0xA0000000, 0x1000); ident = reg.read(0x38)
print("RESULT id=%08x version=%d" % (ident, ident & 255))
NW = 1 << 18; ibuf = allocate(shape=(NW,), dtype=np.uint64); obuf = allocate(shape=(NW,), dtype=np.uint64)
lo = min(ibuf.device_address, obuf.device_address) >> 12; hi = (max(ibuf.device_address, obuf.device_address) + 8 * NW + 4095) >> 12
reg.write(0x28, lo); reg.write(0x2C, hi)
DOM = {"minishake128": 0x01, "minishake256": 0x01}
def model(name, tpl, cend, olen=16):
    w, rate, f, l = ALG[name]; m = bytearray(tpl); D = DOM.get(name, 0x1F)
    while True:
        d = hashlib.shake_256(bytes(m)).digest(olen) if name == "shake256" else ref_xof(bytes(m), olen, w, rate, f, l, D)
        m[48:48 + olen] = d; m[47] = (m[47] + 1) & 255
        if m[47] == cend: return d
def chains(name, tpls, cend, olen=16):
    w, rate, f, l = ALG[name]; dual = w == 32; nb = w // 8; ln = 64; wpm = ln // nb; wpo = olen // nb; n = len(tpls); nj = n // 2 if dual else n
    raw = np.frombuffer(b"".join(tpls), dtype=np.uint8).reshape(n, ln)
    if dual: words = np.ascontiguousarray(raw.reshape(nj, 2, wpm, 4).transpose(0, 2, 1, 3)).reshape(-1).view(np.uint64)
    else: words = np.ascontiguousarray(raw).reshape(-1).view(np.uint64)
    ibuf[:len(words)] = words; ibuf.flush(); obuf[:nj * wpo] = 0; obuf.flush()
    reg.write(0x08, ibuf.device_address & 0xffffffff); reg.write(0x0C, ibuf.device_address >> 32)
    reg.write(0x10, obuf.device_address & 0xffffffff); reg.write(0x14, obuf.device_address >> 32)
    reg.write(0x18, nj); reg.write(0x1C, ln | (DOM.get(name, 0x1F) << 24)); reg.write(0x20, olen)
    reg.write(0x24, f | (l << 8) | (dual << 16) | ((rate // nb) << 24))
    reg.write(0x44, 1 | ((47 // nb) << 8) | ((47 % nb) << 16) | ((48 // nb) << 20)); reg.write(0x48, cend)
    reg.write(0x00, 2); t0 = time.perf_counter(); reg.write(0x00, 1)
    while reg.read(0x04) & 1:
        if time.perf_counter() - t0 > 20: break
    st = reg.read(0x04); ck = reg.read(0x34); reg.write(0x44, 0); obuf.invalidate(); o = np.array(obuf[:nj * wpo]).view(np.uint8)
    if dual: d = np.ascontiguousarray(o.reshape(nj, wpo, 2, 4).transpose(0, 2, 1, 3)).reshape(n, wpo * 4)
    else: d = o.reshape(nj, wpo * 8)
    return [bytes(d[k, :olen]) for k in range(n)], ck, st
bad = 0
# fixed vector from an independent FIPS 205 implementation: SLH-DSA-SHAKE-128s, layer 1, tree 0x1122334455, key pair 7, chain 5, 15 steps
seed = bytes(range(16)); adrs = bytes.fromhex("0000000100000000000000112233445500000000000000070000000500000000"); x = bytes(range(100, 116))
d, ck, st = chains("shake256", [seed + adrs + x], 15); ok = d[0].hex() == "3a666d7625b1d02f484e003f1df7aa4d" and st == 2; bad += not ok
print("RESULT fips205_chain_vector %s status=%02x got=%s" % ("MATCH" if ok else "MISMATCH", st, d[0].hex()))
random.seed(5); rb = lambda n: bytes(random.getrandbits(8) for _ in range(n))
def tpl(start): m = bytearray(rb(64)); m[47] = start; return bytes(m)
for name in ("shake256", "turboshake256", "minishake128"):
    f = 0; total = 0
    for start, cend, n in ((0, 15, 36), (0, 1, 4), (7, 15, 36), (14, 15, 4), (0, 15, 2)):
        tp = [tpl(start) for _ in range(n)]; d, ck, st = chains(name, tp, cend); total += n
        f += sum(a != model(name, t, cend) for a, t in zip(d, tp)) + (st != 2) * n
    bad += f; print("RESULT chain_check alg=%s chains=%d mismatches=%d" % (name, total, f), flush=True)
# time per chain step: 2,000 chains of 15 steps, counted in hardware from go to the last digest in memory
for name in ("shake256", "turboshake256", "minishake128"):
    n = 2000; base = [tpl(0) for _ in range(16)]; tp = (base * (n // 16 + 1))[:n]; d, ck, st = chains(name, tp, 15); steps = 15 * n
    f = sum(d[k] != model(name, tp[k], 15) for k in range(16)) + (st != 2) * 16; bad += f
    print("RESULT chain_time alg=%s chains=%d steps=%d hw_clocks=%d ns_per_step=%.1f ns_per_chain=%.0f mismatches=%d" % (name, n, steps, ck, ck * 1000.0 / BUS / steps, ck * 1000.0 / BUS / n, f), flush=True)
# a normal batch must still work after chain mode
msgs = [rb(32) for _ in range(16)]; raw = np.frombuffer(b"".join(msgs), dtype=np.uint8).view(np.uint64)
ibuf[:len(raw)] = raw; ibuf.flush(); reg.write(0x18, 16); reg.write(0x1C, 32 | (0x1F << 24)); reg.write(0x20, 32); reg.write(0x24, (23 << 8) | (21 << 24)); reg.write(0x00, 2); reg.write(0x00, 1)
t0 = time.time()
while reg.read(0x04) & 1 and time.time() - t0 < 5: pass
obuf.invalidate(); o = bytes(np.array(obuf[:64]).view(np.uint8)); f = sum(o[32*k:32*k+32] != hashlib.shake_128(msgs[k]).digest(32) for k in range(16)); bad += f
print("RESULT plain_batch_after_chain mismatches=%d" % f)
print("RESULT chain summary mismatches=%d %s" % (bad, "FAIL" if bad else "ALL_MATCH"))
