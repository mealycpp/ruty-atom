# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty_kat3.py (NEW, 2026-11-26): ruty_kat.py for one chosen atom of an atom-v3 image. Changes from ruty_kat.py: the atom index is the
# third argument (atom i is at 0xA0000000 + 0x1000 * i), the chain registers 0x44 and 0x48 are cleared, ruty_ref.py is taken from the current
# directory, and the first line also prints the image and the atom. The vectors, the expected values and every check are unchanged.
# RUTY known-answer tests on the board.
#   NIST CAVP SHA-3 XOF byte-oriented vectors (shakebytetestvectors.zip): ShortMsg, LongMsg, VariableOut, Monte Carlo, SHAKE128 and SHAKE256
#   RFC 9861 test vectors: TurboSHAKE128 and TurboSHAKE256
# Every expected value comes from the published file, not from our own model. The model is checked against the same files as well.
# usage: python3 ruty_kat3.py <image>/ruty.bit <dir with the .rsp files and rfc9861.txt> <atom index>
import sys, os, re, time, hashlib, numpy as np
sys.path.insert(0, os.getcwd())
from pynq import Overlay, MMIO, allocate
from ruty_ref import ALG, ref_xof
bit, kd, atom = sys.argv[1], sys.argv[2], int(sys.argv[3])
ol = Overlay(bit); reg = MMIO(0xA0000000 + 0x1000 * atom, 0x1000); ident = reg.read(0x38)
reg.write(0x44, 0); reg.write(0x48, 0)
print("RESULT id=%08x version=%d image=%s atom=%d" % (ident, ident & 255, os.path.basename(os.path.dirname(os.path.abspath(bit))), atom), flush=True)
NW = 1 << 20
ibuf = allocate(shape=(NW,), dtype=np.uint64); obuf = allocate(shape=(NW,), dtype=np.uint64)
lo = min(ibuf.device_address, obuf.device_address) >> 12; hi = (max(ibuf.device_address, obuf.device_address) + 8 * NW + 4095) >> 12
reg.write(0x28, lo); reg.write(0x2C, hi); reg.write(0x30, 1)
def hw(name, msg, olen, D=0x1F):
    w, rate, f, l = ALG[name]; ln = len(msg); wpm = -(-ln // 8); wpo = -(-olen // 8)
    raw = np.zeros(max(wpm, 1) * 8, dtype=np.uint8); raw[:ln] = np.frombuffer(msg, dtype=np.uint8)
    ibuf[:max(wpm, 1)] = raw.view(np.uint64); ibuf.flush(); obuf[:wpo] = 0; obuf.flush()
    reg.write(0x08, ibuf.device_address & 0xffffffff); reg.write(0x0C, ibuf.device_address >> 32)
    reg.write(0x10, obuf.device_address & 0xffffffff); reg.write(0x14, obuf.device_address >> 32)
    reg.write(0x18, 1); reg.write(0x1C, ln | (D << 24)); reg.write(0x20, olen); reg.write(0x24, f | (l << 8) | ((rate // 8) << 24))
    reg.write(0x00, 2); t0 = time.perf_counter(); reg.write(0x00, 1)
    while reg.read(0x04) & 1:
        if time.perf_counter() - t0 > 20: break
    st = reg.read(0x04); obuf.invalidate()
    if st & 0x72 != 2: return b"status%02x" % st
    return bytes(np.array(obuf[:wpo]).view(np.uint8)[:olen])
def sw(name, msg, olen, D=0x1F): w, rate, f, l = ALG[name]; return ref_xof(msg, olen, w, rate, f, l, D)
def fields(path):
    for line in open(path):
        m = re.match(r"\[?\s*([A-Za-z ()]+?)\s*=\s*([0-9a-fA-F]+)\s*\]?\s*$", line.strip())
        if m: yield m.group(1), m.group(2)
tot = bad = 0
def report(what, n, fh, fs):
    global tot, bad; tot += n; bad += fh + fs
    print("RESULT kat %s vectors=%d hardware_mismatches=%d model_mismatches=%d" % (what, n, fh, fs), flush=True)
for bits in (128, 256):
    name = "shake%d" % bits
    for kind in ("ShortMsg", "LongMsg"):
        n = fh = fs = 0; olen = ln = None
        for k, v in fields("%s/SHAKE%d%s.rsp" % (kd, bits, kind)):
            if k == "Outputlen": olen = int(v) // 8
            elif k == "Len": ln = int(v)
            elif k == "Msg": msg = bytes.fromhex(v)[:ln // 8]
            elif k == "Output":
                if ln % 8: continue
                exp = bytes.fromhex(v); n += 1; fh += hw(name, msg, olen) != exp; fs += sw(name, msg, olen) != exp
        report("NIST_%s_%s" % (name, kind), n, fh, fs)
    n = fh = fs = 0
    for k, v in fields("%s/SHAKE%dVariableOut.rsp" % (kd, bits)):
        if k == "Outputlen": olen = int(v) // 8
        elif k == "Msg": msg = bytes.fromhex(v)
        elif k == "Output": exp = bytes.fromhex(v); n += 1; fh += hw(name, msg, olen) != exp; fs += sw(name, msg, olen) != exp
    report("NIST_%s_VariableOut" % name, n, fh, fs)
    # Monte Carlo (SHA3VS 6.3.3): 100 checkpoints, 1000 chained hashes each, output length driven by the previous output
    exp = []; seed = None; mn = mx = None
    for k, v in fields("%s/SHAKE%dMonte.rsp" % (kd, bits)):
        if k.startswith("Minimum"): mn = int(v) // 8
        elif k.startswith("Maximum"): mx = int(v) // 8
        elif k == "Msg": seed = bytes.fromhex(v)
        elif k == "Output": exp.append(bytes.fromhex(v))
    for label, fn in (("hardware", hw), ("model", lambda nm, m, o: hashlib.new("shake_%d" % bits, m).digest(o))):
        out = seed; olen = mx; f = 0
        for j in range(len(exp)):
            for i in range(1000):
                msg = (out + bytes(16))[:16]; out = fn(name, msg, olen); last = olen
                olen = mn + int.from_bytes(out[-2:], "big") % (mx - mn + 1)
            f += out != exp[j]
        if label == "hardware": fh = f
        else: fs = f
    report("NIST_%s_MonteCarlo(100x1000_hashes)" % name, len(exp), fh, fs)
# RFC 9861
txt = open("%s/rfc9861.txt" % kd).read().split("\n"); i = 0; res = {}
def ptn(n): return bytes(k % 251 for k in range(n))
while i < len(txt):
    m = re.match(r"\s*(TurboSHAKE(?:128|256))\(M=(.+?), D=`([0-9A-F]{2})`, (\d+)\)(, last 32 bytes)?:", txt[i]); i += 1
    if not m: continue
    name, mx_, D, olen, last = m.group(1).lower(), m.group(2), int(m.group(3), 16), int(m.group(4)), m.group(5)
    need = 32 if last else olen; h = b""
    while len(h) < need and i < len(txt):
        s = txt[i].strip().strip("`"); i += 1
        if re.fullmatch(r"([0-9A-F]{2} ?)+", s): h += bytes.fromhex(s)
    p = re.match(r"ptn\((\d+)\*\*(\d+) bytes\)", mx_); q = re.match(r"`([0-9A-F ]+)`(?:\^(\d+))?", mx_)
    msg_len = int(p.group(1)) ** int(p.group(2)) if p else None
    key = (name, mx_, D, olen)
    if p and msg_len > 8 * NW - 8: res.setdefault(name, [0, 0, 0, 0])[3] += 1; print("RESULT kat RFC9861 skipped %s M=%s (message larger than the test buffer)" % (name, mx_)); continue
    msg = ptn(msg_len) if p else bytes.fromhex(q.group(1)) * (int(q.group(2)) if q.group(2) is not None else 1)
    a = hw(name, msg, olen, D); b = sw(name, msg, olen, D); r = res.setdefault(name, [0, 0, 0, 0]); r[0] += 1; r[1] += a[-need:] != h; r[2] += b[-need:] != h
for name, r in res.items():
    report("RFC9861_%s" % name, r[0], r[1], r[2])
    if r[3]: print("RESULT kat RFC9861_%s skipped=%d" % (name, r[3]))
print("RESULT kat summary vectors=%d mismatches=%d %s" % (tot, bad, "FAIL" if bad else "ALL_MATCH"))
