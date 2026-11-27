#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""NON-STANDARD variants (TurboSHAKE256 / MiniSHAKE128 in place of SHAKE256): NOT FIPS 205, NOT SLH-DSA, no external reference exists.
Second, independent path for INTERNAL CONSISTENCY only: the pure-Python sources of PyPI slh-dsa 0.2.5 (copied to
C:\\Users\\elhad\\ruty\\pyref_py, no compiled modules) with every SHAKE256 call replaced by the project's own Python sponge ruty_ref.py
(TurboSHAKE256: D=0x1F, rounds 12..23; MiniSHAKE128: Keccak-p[800] rounds 10..21, rate 68 B, D=0x01). Checks PK.root, the signature
bytes, verify, and rejection of a flipped bit. The board program's own software verify (Rust on the A53) is the first path.
Version 2: also the hybrid suite (PRF, F: MiniSHAKE128; H, T_l, H_msg, PRF_msg: TurboSHAKE256) and the firmware variants
(fw-turbo, fw-mini, fw-hybrid: pre-hash TurboSHAKE256-512 of the 1 MB image, M' = 0x01 || 0x00 || 11 zero bytes || digest).
usage: python check_sign_variant2.py <result file> [v ...]"""
import sys, os, hashlib
sys.path.insert(0, r'C:\Users\elhad\ruty\pyref_py')
import ruty_ref
import slhdsa.lowlevel.parameters as P
MODE = {'turbo': ('turboshake256', 0x1F), 'mini': ('minishake128', 0x01)}
class Sponge:
    def __init__(self, name, dom): self.name, self.dom, self.buf = name, dom, bytearray()
    def update(self, b): self.buf += bytes(b)
    def digest(self, n): w, rate, f, l = ruty_ref.ALG[self.name]; return ruty_ref.ref_xof(bytes(self.buf), n, w, rate, f, l, D=self.dom)
def main(path, only):
    keys, sigs = {}, {}
    for line in open(path):
        f = line.split()
        if f and f[0] in ('KEY', 'SIG'):
            d = dict(x.split('=', 1) for x in f[1:]); (keys if f[0] == 'KEY' else sigs)[d['v']] = d
    bad = n = 0
    for v, k in keys.items():
        if only and v not in only: continue
        mode = k['mode'].replace('fw-', '')
        if mode == 'hybrid':      # PRF, F: MiniSHAKE128; H, T_l, H_msg, PRF_msg: TurboSHAKE256 (our non-standard choice)
            def sp(name, dom, parts, n):
                x = Sponge(name, dom)
                for q in parts: x.update(q.to_bytes() if hasattr(q, 'to_bytes') and not isinstance(q, (bytes, bytearray, memoryview)) else q)
                return x.digest(n)
            C = P.ShakeParameter
            C.PRF = lambda self, s, sk, a: sp('minishake128', 0x01, [s, a, sk], self.n)
            C.F = lambda self, s, a, m: sp('minishake128', 0x01, [s, a, m], self.n)
            C.H = lambda self, s, a, m: sp('turboshake256', 0x1F, [s, a, m], self.n)
            C.Tl = lambda self, s, a, m: sp('turboshake256', 0x1F, [s, a, m], self.n)
            C.Hmsg = lambda self, r, s, rt, m: sp('turboshake256', 0x1F, [r, s, rt, m], self.m)
            C.PRFmsg = lambda self, p, o, m: sp('turboshake256', 0x1F, [p, o, m], self.n)
        else:
            name, dom = MODE[mode]; P.shake_256 = lambda: Sponge(name, dom)
        from slhdsa.lowlevel.xmss import XMSS; from slhdsa.lowlevel.addresses import Address; import slhdsa.lowlevel.slhdsa as LL
        par = P.shake_128f
        sks, skp, seed, root = (bytes.fromhex(k[x]) for x in ('sk_seed', 'sk_prf', 'pk_seed', 'pk_root'))
        if v == 'fw':
            x = 0xF1F1_2026_1126_0001; M64 = (1 << 64) - 1; img = bytearray(int(k['image_len']))
            for i in range(len(img)):
                x ^= (x << 13) & M64; x ^= x >> 7; x ^= (x << 17) & M64; img[i] = (x >> 24) & 0xFF
            w, rate, f, l = ruty_ref.ALG['turboshake256']; dig = ruty_ref.ref_xof(bytes(img), 64, w, rate, f, l, D=0x1F)
            if dig.hex() != k['image_turboshake256_512']: print(f"v=fw PREHASH DISAGREES"); bad += 1
            mp = b'\x01\x00' + bytes(11) + dig
        else:
            msg = bytes.fromhex(k['msg']); mp = msg if k.get('iface') == 'internal' else b'\x00\x00' + msg
        r2 = XMSS(par).node(sks, 0, par.h_m, seed, Address(par.d - 1, 0))
        ref = LL.sign(mp, (sks, skp, seed, r2), par, False); s = bytes.fromhex(sigs[v]['hex'])
        vok = LL.verify(mp, s, (seed, root), par)
        fl = bytearray(s); fl[5000] ^= 1; rej = not LL.verify(mp, bytes(fl), (seed, root), par)
        ok = r2 == root and ref == s and vok and rej; n += 1; bad += 0 if ok else 1
        first = None if ref == s else next(i for i in range(len(s)) if s[i] != ref[i])
        print(f"v={v} mode={k['mode']} NON-STANDARD pk_root_agrees={r2 == root} sig_agrees={ref == s}" + (f" FIRST_DIFF_BYTE={first}" if first is not None else "")
              + f" python_verify={vok} python_reject_flip={rej} sig_sha256={hashlib.sha256(s).hexdigest()[:16]} {'AGREE' if ok else 'DISAGREE'}", flush=True)
    print(f"SUMMARY {path} NON-STANDARD checked={n} disagree={bad}")
if __name__ == '__main__': main(sys.argv[1], sys.argv[2:])
