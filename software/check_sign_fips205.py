#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""Independent check of ruty_sign output (board logs) against PyPI slh-dsa 0.2.5 (pure-Python FIPS 205 implementation, installed in
C:\\Users\\elhad\\ruty\\pyref; validated separately against all 16 NIST ACVP SLH-DSA-SHAKE-128f deterministic sigGen vectors).
For every KEY/SIG pair: PK.root from slh-dsa's own XMSS (key generation), the signature from slh-dsa sign_pure (deterministic,
opt_rand = PK.seed, empty context), sign (internal interface, for the ACVP vector) or sign_hash (pre-hash SHAKE256-512, OID 2.16.840.1.101.3.4.2.12),
byte compare, slh-dsa verify of the board's signature, rejection of the same six flipped-bit signatures and of a wrong message.
The ACVP vector (v=3) is also compared with NIST's expected signature by SHA-256: 3f67c5420ba7d1b58c1149e260843dd658f0ff3049326ec246adb4697f8c6419.
usage: set PYTHONPATH=C:\\Users\\elhad\\ruty\\pyref & python check_sign_fips205.py <result file>"""
import sys, hashlib
import slhdsa
from slhdsa.lowlevel.parameters import shake_128f as PAR
from slhdsa.lowlevel.xmss import XMSS
from slhdsa.lowlevel.addresses import Address

N, FK, FA, WL, HP = 16, 33, 6, 35, 3
SIGLEN = 17088; HTOFF = N * (1 + FK * (FA + 1))
FLIPS = [0, N + 5, N + FK * (FA + 1) * N - 1, HTOFF + 100, HTOFF + 11 * (WL + HP) * N + 7, SIGLEN - 1]
ACVP_SHA256 = '3f67c5420ba7d1b58c1149e260843dd658f0ff3049326ec246adb4697f8c6419'   # NIST ACVP internalProjection tg16 tc137 expected signature
OID_PREFIX = b'\x06\x09\x60\x86\x48\x01\x65\x03\x04\x02'

def rng_bytes(seed, n):
    x = seed; out = bytearray(n); M = (1 << 64) - 1
    for i in range(n):
        x ^= (x << 13) & M; x ^= x >> 7; x ^= (x << 17) & M; out[i] = (x >> 24) & 0xFF
    return bytes(out)

def ph_shake256(data):
    return OID_PREFIX + b'\x0c' + hashlib.shake_256(data).digest(64)

def main(path):
    keys, sigs = {}, {}
    for line in open(path):
        f = line.split()
        if not f: continue
        if f[0] == 'KEY':
            d = dict(x.split('=', 1) for x in f[1:]); keys[d['v']] = d
        elif f[0] == 'SIG':
            d = dict(x.split('=', 1) for x in f[1:]); sigs[d['v']] = bytes.fromhex(d['hex'])
    bad = 0; n = 0
    for v, k in keys.items():
        if k.get('mode') not in ('std', 'fw'):
            print(f"v={v}: mode {k.get('mode')} is NOT FIPS 205; skipped here"); continue
        sks, skp, seed, root = (bytes.fromhex(k[x]) for x in ('sk_seed', 'sk_prf', 'pk_seed', 'pk_root'))
        ref_root = XMSS(PAR).node(sks, 0, PAR.h_m, seed, Address(PAR.d - 1, 0))
        sk = slhdsa.SecretKey((sks, skp, seed, ref_root), slhdsa.shake_128f); pk = sk.pubkey
        extra = ''
        if v == 'fw':
            img = rng_bytes(0xF1F1_2026_1126_0001, int(k['image_len']))
            img_ok = hashlib.shake_256(img).digest(64).hex() == k['image_shake256_512']
            ref = sk.sign_hash(img, randomize=False, prehash=ph_shake256)
            good_msg = img; wrong = bytearray(img); wrong[123457] ^= 0x10; wrong = bytes(wrong)
            ver = lambda m, s: pk.verify_hash(m, s, prehash=ph_shake256)
        elif k.get('iface') == 'internal':
            msg = bytes.fromhex(k['msg']); img_ok = len(msg) == int(k['msg_len'])
            ref = sk.sign(msg, randomize=False)
            h = hashlib.sha256(sigs.get(v, b'')).hexdigest(); img_ok = img_ok and h == ACVP_SHA256; extra = f" equals_NIST_ACVP_signature={h == ACVP_SHA256}"
            good_msg = msg; wrong = msg[:-1] + bytes([msg[-1] ^ 1])
            ver = lambda m, s: pk.verify(m, s)
        else:
            msg = bytes.fromhex(k['msg']); img_ok = len(msg) == int(k['msg_len'])
            ref = sk.sign_pure(msg, randomize=False)
            good_msg = msg; wrong = (msg[:-1] + bytes([msg[-1] ^ 1])) if msg else b'\x00'
            ver = lambda m, s: pk.verify_pure(m, s)
        s = sigs.get(v, b'')
        same = s == ref
        first = next((i for i in range(min(len(s), len(ref))) if s[i] != ref[i]), None) if not same else None
        vok = ver(good_msg, s)
        rej = sum(1 for p in FLIPS if not ver(good_msg, bytes(b ^ ((1 << (p % 8)) if i == p else 0) for i, b in enumerate(s))))
        rejm = not ver(wrong, s)
        ok = img_ok and root == ref_root and same and vok and rej == len(FLIPS) and rejm and len(s) == SIGLEN
        n += 1; bad += 0 if ok else 1
        print(f"v={v} mode={k['mode']} pk_root_identical={root == ref_root} sig_len={len(s)} sig_identical={same}"
              + (f" FIRST_DIFF_BYTE={first}" if first is not None else "") + extra
              + f" slhdsa_verify_board_sig={vok} slhdsa_reject_flipped={rej}/{len(FLIPS)} slhdsa_reject_wrong_msg={rejm} input_ok={img_ok}"
              + f" sig_sha256={hashlib.sha256(s).hexdigest()[:16]} {'PASS' if ok else 'FAIL'}")
    print(f"SUMMARY {path} checked={n} failed={bad} {'ALL_IDENTICAL_AND_VERIFIED' if bad == 0 and n > 0 else 'FAIL'}")

if __name__ == '__main__':
    main(sys.argv[1])
