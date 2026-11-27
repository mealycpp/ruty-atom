# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty_pqc: hash kernels of hash-based signatures on RUTY (chains and Merkle tree), driven by the Rust driver, plus the
# per-hash table. Every result is checked against software.
#   sudo bash -c "source /etc/profile.d/pynq_venv.sh; source /etc/profile.d/xrt_setup.sh; python3 ruty_pqc.py ruty.bit [bus MHz]"
import sys, ctypes, hashlib
import numpy as np
from pynq import Overlay, MMIO, allocate
exec(open("ruty_ref.py").read())                      # rol, perm, ref_xof, ALG, soft
bit = sys.argv[1] if len(sys.argv) > 1 else "ruty.bit"; BUS = float(sys.argv[2]) if len(sys.argv) > 2 else 300.0
ol = Overlay(bit); reg = MMIO(0xA0000000, 0x1000); ident = reg.read(0x38)
print("RESULT id=%08x slices_per_clock=%d version=%d" % (ident, (ident >> 8) & 255, ident & 255))
drv = ctypes.CDLL("./libruty_drv.so"); u64, u32, dbl = ctypes.c_uint64, ctypes.c_uint32, ctypes.c_double
drv.ruty_open.argtypes = [u64]; assert drv.ruty_open(0xA0000000) == 0
for f, at in ((drv.ruty_chains, [u64, u64, u32, u32, u32, u32, ctypes.POINTER(u64)]), (drv.ruty_tree, [u64, u64, u32, u32, u32, ctypes.POINTER(u64)]),
              (drv.ruty_batch, [u64, u64, u32, u32, u32, u32, ctypes.POINTER(u64)])): f.argtypes = at; f.restype = dbl
NW = 1 << 20
A = allocate(shape=(NW,), dtype=np.uint64); B = allocate(shape=(NW,), dtype=np.uint64)
lo = min(A.device_address, B.device_address) >> 12; hi = (max(A.device_address, B.device_address) + 8 * NW + 4095) >> 12
reg.write(0x28, lo); reg.write(0x2C, hi); reg.write(0x30, 1)
def algword(name): w, rate, f, l = ALG[name]; return f | (l << 8) | ((w == 32) << 16) | ((rate // (w // 8)) << 24)
def pack(msgs, dual):                                 # equal-length messages -> 64-bit words (dual: pairs interleaved)
    nb = 4 if dual else 8; ln = len(msgs[0]); wpm = -(-ln // nb); raw = np.zeros((len(msgs), wpm * nb), dtype=np.uint8)
    for k, m in enumerate(msgs): raw[k, :ln] = np.frombuffer(m, dtype=np.uint8)
    if dual: return np.ascontiguousarray(raw.reshape(len(msgs) // 2, 2, wpm, 4).transpose(0, 2, 1, 3)).reshape(-1).view(np.uint64)
    return raw.reshape(-1).view(np.uint64)
def unpack(buf, n, olen, dual):                       # n digests of olen bytes
    nb = 4 if dual else 8; wpo = -(-olen // nb); nj = n // 2 if dual else n; o = np.array(buf[:nj * wpo]).view(np.uint8)
    d = np.ascontiguousarray(o.reshape(nj, wpo, 2, 4).transpose(0, 2, 1, 3)).reshape(n, wpo * 4) if dual else o.reshape(nj, wpo * 8)
    return [bytes(d[k, :olen]) for k in range(n)]
def put(buf, words): buf[:len(words)] = words; buf.flush()
bad = 0; ck = u64(0)
SW = {("chains", "shake128"): 1784163.0, ("chains", "turboshake128"): 974761.0, ("chains", "minishake128"): 957632.0,
      ("merkle", "shake128"): 916951.0, ("merkle", "turboshake128"): 587461.0, ("merkle", "minishake128"): 510331.0}   # ns, same board, plain C
for name in ("shake128", "turboshake128", "minishake128"):
    dual = ALG[name][0] == 32; aw = algword(name)
    # ---- hash chains: 67 chains (dual: 68, an even number) of 15 steps, x = H(x), 32 bytes ----
    nch = 68 if dual else 67; seeds = [bytes([i]) * 32 for i in range(nch)]; nj = nch // 2 if dual else nch
    put(A, pack(seeds, dual)); B[:nj * 8] = 0; B.flush()
    t = drv.ruty_chains(A.device_address, B.device_address, nj, 32, aw, 15, ctypes.byref(ck)); B.invalidate(); A.invalidate()
    got = unpack(B, nch, 32, dual); exp = []
    for s in seeds:
        x = s
        for _ in range(15): x = soft(name, x, 32)
        exp.append(x)
    f = sum(g != e for g, e in zip(got, exp)) + (t < 0); bad += f
    t = drv.ruty_chains(A.device_address, B.device_address, nj, 32, aw, 15 * 200, ctypes.byref(ck)) / 200; hw = ck.value / 200 * 1000.0 / BUS
    sw = SW[("chains", name)] * (nch * 15) / 1005.0
    print("RESULT app=chains alg=%s chains=%d steps=15 hashes=%d ruty_ns=%.0f hw_ns=%.0f software_ns=%.0f gain=%.1f mismatches=%d" % (name, nch, nch * 15, t, hw, sw, sw / t, f))
    # ---- Merkle tree, 256 leaves of 32 bytes: one core builds the tree; two cores build the two half-trees side by side ----
    leaves = [bytes([i]) * 32 for i in range(256)]
    if dual: order = [leaves[i // 2 + (128 if i % 2 else 0)] for i in range(256)]            # pair k = (leaf k, leaf 128+k)
    else: order = leaves
    put(A, pack(order, dual)); B[:256 * 8] = 0; B.flush(); nl = 128 if dual else 256
    t = drv.ruty_tree(A.device_address, B.device_address, nl, aw, 1, ctypes.byref(ck)); A.invalidate(); B.invalidate()
    nb = 1 + (7 if dual else 8); res = B if nb % 2 else A                                     # buffer that holds the last digests
    lvl = [soft(name, x, 32) for x in leaves]
    while len(lvl) > (2 if dual else 1): lvl = [soft(name, lvl[2*i] + lvl[2*i+1], 32) for i in range(len(lvl) // 2)]
    got = unpack(res, 2 if dual else 1, 32, dual); f = (got != lvl) + (t < 0); bad += f
    t = drv.ruty_tree(A.device_address, B.device_address, nl, aw, 200, ctypes.byref(ck)) / 200; hw = ck.value / 200 * 1000.0 / BUS
    extra = 971.2 if dual else 0.0                                                             # the root of the two half-trees: one software hash (measured, 64 bytes)
    sw = SW[("merkle", name)]
    print("RESULT app=merkle alg=%s leaves=256 hashes=511 on_ruty=%d ruty_ns=%.0f hw_ns=%.0f root_sw_ns=%.0f software_ns=%.0f gain=%.1f mismatches=%d" % (name, 510 if dual else 511, t, hw, extra, sw, sw / (t + extra), f))
# ---- per-hash table, 20,000-hash batches, Rust driver ----
import random
random.seed(3); rb = lambda n: bytes(random.getrandbits(8) for _ in range(n))
for name, ln, n in (("shake128", 32, 20000), ("shake128", 64, 20000), ("shake128", 136, 20000), ("shake128", 4096, 1000), ("shake256", 32, 20000), ("turboshake128", 32, 20000),
                    ("turboshake128", 4096, 1000), ("minishake128", 32, 20000), ("minishake128", 64, 20000), ("minishake256", 32, 20000)):
    dual = ALG[name][0] == 32; base = [rb(ln) for _ in range(16)]; msgs = (base * (n // 16 + 1))[:n]; nj = n // 2 if dual else n
    put(A, pack(msgs, dual)); B[:nj * 8] = 0; B.flush()
    t = drv.ruty_batch(A.device_address, B.device_address, nj, ln, 32, algword(name), ctypes.byref(ck)); B.invalidate()
    got = unpack(B, n, 32, dual); f = sum(got[k] != soft(name, msgs[k], 32) for k in list(range(16)) + list(range(n - 16, n))) + (t < 0); bad += f
    print("RESULT time alg=%s len=%d jobs=%d hw_ns_per_hash=%.1f program_ns_per_hash=%.1f mismatches=%d" % (name, ln, n, ck.value * 1000.0 / BUS / n, t / n, f))
print("RESULT summary mismatches=%d %s" % (bad, "FAIL" if bad else "ALL_MATCH"))
