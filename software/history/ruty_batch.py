# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty_batch: load the RUTY bus-master image and run batches of hashes. The ARM writes a few registers per batch;
# RUTY fetches the messages from memory and stores the digests itself. Every digest checked is compared with software.
#   sudo bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 ruty_batch.py ruty.bit [bus MHz]"
import sys, time, random, hashlib
import numpy as np
from pynq import Overlay, MMIO, allocate
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
bit = sys.argv[1] if len(sys.argv) > 1 else "ruty.bit"; BUS = float(sys.argv[2]) if len(sys.argv) > 2 else 300.0
ol = Overlay(bit); reg = MMIO(0xA0000000, 0x1000); ident = reg.read(0x38)
print("RESULT id=%08x slices_per_clock=%d version=%d" % (ident, (ident >> 8) & 255, ident & 255))
if ident >> 16 != 0x5255: sys.exit("RESULT error=engine_not_found")
NW = 1 << 20                                             # 8 MB each way
ibuf = allocate(shape=(NW,), dtype=np.uint64); obuf = allocate(shape=(NW,), dtype=np.uint64)
lo = min(ibuf.device_address, obuf.device_address) >> 12; hi = (max(ibuf.device_address, obuf.device_address) + 8 * NW + 4095) >> 12
reg.write(0x28, lo); reg.write(0x2C, hi); reg.write(0x30, 1)        # the master may touch these two buffers only; locked until reset
print("RESULT window pages=%x..%x locked=%d" % (lo, hi, reg.read(0x30)))
def batch(name, msgs, olen):
    """msgs: equal-length messages; for the 32-bit-lane functions an even number, paired two by two. Returns digests, clocks, status."""
    w, rate, f, l = ALG[name]; dual = w == 32; nb = w // 8; ln = len(msgs[0]); wpm = -(-ln // nb); wpo = -(-olen // nb)
    nj = len(msgs) // 2 if dual else len(msgs); raw = np.zeros((len(msgs), wpm * nb), dtype=np.uint8)
    for k, m in enumerate(msgs): raw[k, :ln] = np.frombuffer(m, dtype=np.uint8)
    if dual:                                             # one word = 4 bytes of the core-0 job (low half) and 4 of the core-1 job
        a = raw.reshape(nj, 2, wpm, 4); words = np.ascontiguousarray(a.transpose(0, 2, 1, 3)).reshape(-1).view(np.uint64)
    else: words = raw.reshape(-1).view(np.uint64)
    ibuf[:len(words)] = words; ibuf.flush(); obuf[:nj * wpo] = 0; obuf.flush()
    reg.write(0x08, ibuf.device_address & 0xffffffff); reg.write(0x0C, ibuf.device_address >> 32)
    reg.write(0x10, obuf.device_address & 0xffffffff); reg.write(0x14, obuf.device_address >> 32)
    reg.write(0x18, nj); reg.write(0x1C, ln | (0x1F << 24)); reg.write(0x20, olen); reg.write(0x24, f | (l << 8) | (dual << 16) | ((rate // nb) << 24))
    reg.write(0x00, 2); t0 = time.perf_counter(); reg.write(0x00, 1)
    while reg.read(0x04) & 1:
        if time.perf_counter() - t0 > 20: break
    t1 = time.perf_counter(); st = reg.read(0x04); ck = reg.read(0x34); obuf.invalidate()
    o = np.array(obuf[:nj * wpo]).view(np.uint8)
    if dual: d = np.ascontiguousarray(o.reshape(nj, wpo, 2, 4).transpose(0, 2, 1, 3)).reshape(len(msgs), wpo * 4)
    else: d = o.reshape(nj, wpo * 8)
    return [bytes(d[k, :olen]) for k in range(len(msgs))], ck, st, t1 - t0
random.seed(3); rb = lambda n: bytes(random.getrandbits(8) for _ in range(n)); bad = total = 0
for name in ALG:
    fail = 0
    for ln, olen, n in ((0, 32, 4), (1, 32, 4), (32, 32, 64), (31, 64, 8), (67, 33, 8), (68, 200, 4), (135, 32, 4), (136, 32, 4), (167, 32, 4), (168, 500, 4), (169, 32, 4), (1000, 32, 4), (4096, 32, 4)):
        msgs = [rb(ln) for _ in range(n)]; d, ck, st, t = batch(name, msgs, olen); total += n
        fail += sum(x != soft(name, m, olen) for x, m in zip(d, msgs)) + (st != 2) * n
    bad += fail; print("RESULT check alg=%s shapes=13 mismatches=%d" % (name, fail))
# time per hash for large batches: counted in hardware (bus clocks from go to the last digest in memory) and seen by the program
for name, ln, n in (("shake128", 32, 20000), ("shake128", 64, 20000), ("shake128", 136, 20000), ("shake128", 4096, 1000), ("shake256", 32, 20000),
                    ("turboshake128", 32, 20000), ("turboshake128", 4096, 1000), ("minishake128", 32, 20000), ("minishake128", 64, 20000), ("minishake256", 32, 20000)):
    msgs = [rb(ln)] * 4 + [rb(ln) for _ in range(12)]; msgs = (msgs * (n // 16 + 1))[:n]; d, ck, st, t = batch(name, msgs, 32); total += 16
    f = sum(d[k] != soft(name, msgs[k], 32) for k in list(range(8)) + list(range(n - 8, n))) + (st != 2) * 16; bad += f
    print("RESULT time alg=%s len=%d jobs=%d hw_clocks=%d hw_ns_per_hash=%.1f program_ns_per_hash=%.1f mismatches=%d" % (name, ln, n, ck, ck * 1000.0 / BUS / n, t * 1e9 / n, f))
# the window: a batch whose output address is outside it must be refused, then a normal batch must still work
reg.write(0x10, 0x1000); reg.write(0x14, 0); reg.write(0x18, 4); reg.write(0x00, 2); reg.write(0x00, 1); time.sleep(0.05); st = reg.read(0x04)
print("RESULT window_refusal status=%02x %s" % (st, "REFUSED" if st & 0x40 else "NOT_REFUSED")); bad += not (st & 0x40)
msgs = [rb(32) for _ in range(16)]; d, ck, st, t = batch("shake128", msgs, 32); f = sum(x != soft("shake128", m, 32) for x, m in zip(d, msgs)); bad += f
print("RESULT after_refusal mismatches=%d" % f)
print("RESULT summary checks=%d mismatches=%d %s" % (total, bad, "FAIL" if bad else "ALL_MATCH"))
