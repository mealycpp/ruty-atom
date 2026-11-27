# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# check_modes2.py (NEW, made by mk_check_modes2.py from check_modes.py): independent check of ruty_modes result files (KT128 digests, flat digests, Merkle roots).
#   python check_modes.py <pyref_modes folder with pycryptodome> <result file> [more result files]
# KT128 and TurboSHAKE128: pycryptodome (Crypto.Hash.KangarooTwelve, TurboSHAKE128); SHAKE128: Python hashlib (OpenSSL).
# MiniSHAKE128: no independent implementation exists anywhere; here a pure-Python Keccak-p[b] written for this checker, w in {32, 64},
# whose w=64 path is first checked against hashlib SHAKE128 and pycryptodome TurboSHAKE128 (same code, only the lane width differs).
import sys, re, hashlib
sys.path.insert(0, sys.argv[1])
from Crypto.Hash import KangarooTwelve as K12, TurboSHAKE128
class Rng:
    def __init__(s, seed): s.s = seed
    def msg(s, n):
        out = bytearray(n); x = s.s; M = (1 << 64) - 1
        for i in range(n):
            x ^= (x << 13) & M; x ^= x >> 7; x ^= (x << 17) & M; out[i] = (x >> 24) & 255
        s.s = x; return bytes(out)
RHO = [0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14]
def _lfsr_rc():
    # FIPS 202 rc(t) bit generator, independent of the Rust code's loop structure
    def rcbit(t):
        if t % 255 == 0: return 1
        R = [1, 0, 0, 0, 0, 0, 0, 0]
        for _ in range(1, t % 255 + 1):
            R = [0] + R
            R[0] ^= R[8]; R[4] ^= R[8]; R[5] ^= R[8]; R[6] ^= R[8]; R = R[:8]
        return R[0]
    out = []
    for ir in range(24):
        v = 0
        for j in range(7): v |= rcbit(j + 7 * ir) << ((1 << j) - 1)
        out.append(v)
    return out
RC = _lfsr_rc()
def perm(A, w, first, last):
    m = (1 << w) - 1
    rot = lambda x, n: ((x << (n % w)) | (x >> (w - n % w))) & m if n % w else x
    for r in range(first, last + 1):
        C = [A[x] ^ A[x + 5] ^ A[x + 10] ^ A[x + 15] ^ A[x + 20] for x in range(5)]
        D = [C[(x - 1) % 5] ^ rot(C[(x + 1) % 5], 1) for x in range(5)]
        A = [A[i] ^ D[i % 5] for i in range(25)]
        B = [0] * 25
        for x in range(5):
            for y in range(5): B[y + 5 * ((2 * x + 3 * y) % 5)] = rot(A[x + 5 * y], RHO[x + 5 * y])
        A = [B[i] ^ ((~B[(i % 5 + 1) % 5 + 5 * (i // 5)]) & m & B[(i % 5 + 2) % 5 + 5 * (i // 5)]) for i in range(25)]
        A[0] ^= RC[r] & m
    return A
def sponge(w, rate, first, last, dom, msg, olen):
    nb = w // 8; st = bytearray(25 * nb)
    def p():
        A = [int.from_bytes(st[nb * i:nb * i + nb], 'little') for i in range(25)]
        A = perm(A, w, first, last)
        for i in range(25): st[nb * i:nb * i + nb] = A[i].to_bytes(nb, 'little')
    i = 0
    while i + rate <= len(msg):
        for j in range(rate): st[j] ^= msg[i + j]
        p(); i += rate
    for j in range(len(msg) - i): st[j] ^= msg[i + j]
    st[len(msg) - i] ^= dom; st[rate - 1] ^= 0x80
    out = b''
    while len(out) < olen: p(); out += bytes(st[:rate])
    return out[:olen]
mini128 = lambda m: sponge(32, 68, 10, 21, 0x01, m, 32)
turbo128 = lambda m: TurboSHAKE128.new(data=m, domain=0x1F).read(32)
shake128 = lambda m: hashlib.shake_128(m).digest(32)
ok_all = True
def chk(name, a, b):
    global ok_all
    ok = a == b; ok_all &= ok; print(('OK   ' if ok else 'FAIL ') + name + ('' if ok else ' board=%s python=%s' % (a, b))); return ok
# self-check of the pure-Python sponge code against independent implementations (w = 64), and of pycryptodome K12 against RFC 9861
for m in [b'', b'abc', bytes(range(200)) * 3]:
    chk('selfcheck pure-python w=64 SHAKE128 len=%d vs hashlib' % len(m), sponge(64, 168, 0, 23, 0x1F, m, 32), shake128(m))
    chk('selfcheck pure-python w=64 TurboSHAKE128 len=%d vs pycryptodome' % len(m), sponge(64, 168, 12, 23, 0x1F, m, 32), turbo128(m))
def ptn(n): return bytes(i % 251 for i in range(n))
chk('selfcheck pycryptodome KT128 RFC9861 ptn(17^3)', K12.new(data=ptn(17 ** 3)).read(32).hex(), 'cb552e2ec77d9910701d578b457ddf772c12e322e4ee7fe417f92c758f0d59d0')
chk('selfcheck pycryptodome KT128 RFC9861 ptn(8192),C=ptn(8190)', K12.new(data=ptn(8192), custom=ptn(8190)).read(32).hex(), '6a7c1b6a5cd0d8c9ca943a4a216cc64604559a2ea45f78570a15253d67ba00ae')
stream = None; vals = {}; roots = {}
def proot(k, alg):
    if (k, alg) not in roots:
        H = {'minishake128': mini128, 'turboshake128': turbo128, 'shake128': shake128}[alg]
        lv = [H(vals[k][32 * i:32 * i + 32]) for i in range(1 << k)]
        while len(lv) > 1: lv = [H(lv[2 * i] + lv[2 * i + 1]) for i in range(len(lv) // 2)]
        roots[(k, alg)] = lv[0].hex()
    return roots[(k, alg)]
for fn in sys.argv[2:]:
    print('== ' + fn)
    for line in open(fn, encoding='utf-8', errors='replace'):
        if line.startswith('RESULT ktdig'):
            if stream is None: stream = Rng(0x4B54_3132_5F4D_5347).msg(16 << 20)
            d = dict(kv.split('=', 1) for kv in line.split()[2:] if '=' in kv); ml = int(d['mlen']); fl = int(d['flat_len']); m = stream[:ml]
            chk('%s KT128 mlen=%d vs pycryptodome KangarooTwelve' % (fn, ml), d['kt128'], K12.new(data=m).read(32).hex())
            chk('%s flat SHAKE128 len=%d vs hashlib' % (fn, fl), d['shake128'], shake128(m[:fl]).hex())
            chk('%s flat TurboSHAKE128 len=%d vs pycryptodome' % (fn, fl), d['turboshake128'], turbo128(m[:fl]).hex())
        if line.startswith('RESULT merkle_root'):
            d = dict(kv.split('=', 1) for kv in line.split()[2:] if '=' in kv); k = int(d['k'])
            if k not in vals: vals[k] = Rng(0x4D45_524B_4C45_0000 + k).msg(32 << k)
            chk('%s Merkle k=%d %s root vs Python (%s)' % (fn, k, d['mode'], 'pure-Python Keccak-p[800]' if d['alg'] == 'minishake128' else 'pycryptodome' if d['alg'] == 'turboshake128' else 'hashlib'), d['sw_root'], proot(k, d['alg']))
        if line.startswith('RESULT swmerkle') or line.startswith('RESULT xkcpmerkle'):
            d = dict(kv.split('=', 1) for kv in line.split()[2:] if '=' in kv); k = int(d['k'])
            if k not in vals: vals[k] = Rng(0x4D45_524B_4C45_0000 + k).msg(32 << k)
            chk('%s %s k=%d %s threads=%s root vs Python' % (fn, line.split()[1], k, d['alg'], d['threads']), d['root'], proot(k, d['alg']))
        if line.startswith('RESULT swkt'):
            if stream is None: stream = Rng(0x4B54_3132_5F4D_5347).msg(16 << 20)
            d = dict(kv.split('=', 1) for kv in line.split()[2:] if '=' in kv); ml = int(d['mlen'])
            chk('%s A53 KT128 mlen=%d threads=%s vs pycryptodome' % (fn, ml, d['threads']), d['kt128'], K12.new(data=stream[:ml]).read(32).hex())
        if line.startswith('RESULT swflat'):
            if stream is None: stream = Rng(0x4B54_3132_5F4D_5347).msg(16 << 20)
            d = dict(kv.split('=', 1) for kv in line.split()[2:] if '=' in kv); fl = int(d['mlen'])
            chk('%s A53 flat %s len=%d vs Python' % (fn, d['alg'], fl), d['digest'], (shake128 if d['alg'] == 'shake128' else turbo128)(stream[:fl]).hex())
        if 'FAILED' in line or 'mismatches=' in line and 'mismatches=0' not in line:
            ok_all = False; print('FAIL board line: ' + line.strip()[:200])
print('CHECK_SUMMARY ' + ('ALL_OK' if ok_all else 'FAILURES'))
