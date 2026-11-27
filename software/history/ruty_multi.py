# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty_multi: N RUTY engines sharing one memory port. For k = 1..N engines running at once, the time per hash of each
# engine is read from its own hardware counter (bus clocks from its go to its last digest in memory).
#   sudo bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 ruty_multi.py ruty.bit N [bus MHz]"
import sys, time, random, hashlib
import numpy as np
from pynq import Overlay, MMIO, allocate
exec(open("ruty_ref.py").read())                      # ref_xof, ALG, soft
bit = sys.argv[1]; N = int(sys.argv[2]); BUS = float(sys.argv[3]) if len(sys.argv) > 3 else 300.0
ol = Overlay(bit); regs = [MMIO(0xA0000000 + 0x1000 * i, 0x1000) for i in range(N)]
print("RESULT engines=%d ids=%s" % (N, " ".join("%08x" % r.read(0x38) for r in regs)))
NW = 1 << 18
bufs = [(allocate(shape=(NW,), dtype=np.uint64), allocate(shape=(NW,), dtype=np.uint64)) for _ in range(N)]
for r, (a, b) in zip(regs, bufs):
    lo = min(a.device_address, b.device_address) >> 12; hi = (max(a.device_address, b.device_address) + 8 * NW + 4095) >> 12
    r.write(0x28, lo); r.write(0x2C, hi); r.write(0x30, 1)
def pack(msgs, dual):
    nb = 4 if dual else 8; ln = len(msgs[0]); wpm = -(-ln // nb); raw = np.zeros((len(msgs), wpm * nb), dtype=np.uint8)
    for k, m in enumerate(msgs): raw[k, :ln] = np.frombuffer(m, dtype=np.uint8)
    if dual: return np.ascontiguousarray(raw.reshape(len(msgs) // 2, 2, wpm, 4).transpose(0, 2, 1, 3)).reshape(-1).view(np.uint64)
    return raw.reshape(-1).view(np.uint64)
def unpack(buf, n, olen, dual):
    nb = 4 if dual else 8; wpo = -(-olen // nb); nj = n // 2 if dual else n; o = np.array(buf[:nj * wpo]).view(np.uint8)
    d = np.ascontiguousarray(o.reshape(nj, wpo, 2, 4).transpose(0, 2, 1, 3)).reshape(n, wpo * 4) if dual else o.reshape(nj, wpo * 8)
    return [bytes(d[k, :olen]) for k in range(n)]
random.seed(5); rb = lambda n: bytes(random.getrandbits(8) for _ in range(n)); bad = 0
for name, ln, n in (("shake128", 32, 40000), ("turboshake128", 32, 40000), ("minishake128", 32, 40000), ("shake128", 4096, 400)):
    w, rate, f, l = ALG[name]; dual = w == 32; nb = w // 8; nj = n // 2 if dual else n
    aw = f | (l << 8) | (dual << 16) | ((rate // nb) << 24)
    sets = []
    for i in range(N):
        base = [rb(ln) for _ in range(16)]; msgs = (base * (n // 16 + 1))[:n]; sets.append(msgs)
        a, b = bufs[i]; words = pack(msgs, dual); a[:len(words)] = words; a.flush()
        r = regs[i]; r.write(0x08, a.device_address & 0xffffffff); r.write(0x0C, a.device_address >> 32)
        r.write(0x10, b.device_address & 0xffffffff); r.write(0x14, b.device_address >> 32)
        r.write(0x18, nj); r.write(0x1C, ln | (0x1F << 24)); r.write(0x20, 32); r.write(0x24, aw)
    for k in range(1, N + 1):
        for i in range(k): bufs[i][1][:nj * 4] = 0; bufs[i][1].flush(); regs[i].write(0x00, 2)
        t0 = time.perf_counter()
        for i in range(k): regs[i].write(0x00, 1)
        while any(regs[i].read(0x04) & 1 for i in range(k)):
            if time.perf_counter() - t0 > 20: break
        t1 = time.perf_counter(); per = []; f_ = 0
        for i in range(k):
            st = regs[i].read(0x04); ck = regs[i].read(0x34); per.append(ck * 1000.0 / BUS / n); bufs[i][1].invalidate()
            got = unpack(bufs[i][1], n, 32, dual)
            f_ += sum(got[j] != soft(name, sets[i][j], 32) for j in list(range(16)) + list(range(n - 16, n))) + (st & 0x72 != 2)
        bad += f_
        print("RESULT multi alg=%s len=%d active=%d ns_per_hash_each=%s mean=%.1f wall_ns_per_hash_all=%.1f mismatches=%d" % (name, ln, k, "/".join("%.1f" % p for p in per), sum(per) / k, (t1 - t0) * 1e9 / (n * k), f_))
print("RESULT summary mismatches=%d %s" % (bad, "FAIL" if bad else "ALL_MATCH"))
