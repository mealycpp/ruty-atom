# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# makes check_sign_variant2.py (NEW) from check_sign_variant.py: adds the hybrid suite and the firmware variants (fw-turbo, fw-mini, fw-hybrid)
import sys
s = open(sys.argv[1]).read()
def R(a, b):
    global s
    assert s.count(a) == 1, a[:60]; s = s.replace(a, b)
R('usage: python check_sign_variant.py <result file> [v ...]"""', '''Version 2: also the hybrid suite (PRF, F: MiniSHAKE128; H, T_l, H_msg, PRF_msg: TurboSHAKE256) and the firmware variants
(fw-turbo, fw-mini, fw-hybrid: pre-hash TurboSHAKE256-512 of the 1 MB image, M' = 0x01 || 0x00 || 11 zero bytes || digest).
usage: python check_sign_variant2.py <result file> [v ...]"""''')
R("""        name, dom = MODE[k['mode']]; P.shake_256 = lambda: Sponge(name, dom)
""", """        mode = k['mode'].replace('fw-', '')
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
""")
R("""        msg = bytes.fromhex(k['msg']); mp = msg if k.get('iface') == 'internal' else b'\\x00\\x00' + msg""",
"""        if v == 'fw':
            x = 0xF1F1_2026_1126_0001; M64 = (1 << 64) - 1; img = bytearray(int(k['image_len']))
            for i in range(len(img)):
                x ^= (x << 13) & M64; x ^= x >> 7; x ^= (x << 17) & M64; img[i] = (x >> 24) & 0xFF
            w, rate, f, l = ruty_ref.ALG['turboshake256']; dig = ruty_ref.ref_xof(bytes(img), 64, w, rate, f, l, D=0x1F)
            if dig.hex() != k['image_turboshake256_512']: print(f"v=fw PREHASH DISAGREES"); bad += 1
            mp = b'\\x01\\x00' + bytes(11) + dig
        else:
            msg = bytes.fromhex(k['msg']); mp = msg if k.get('iface') == 'internal' else b'\\x00\\x00' + msg""")
open(sys.argv[2], 'w').write(s); print('ok')
