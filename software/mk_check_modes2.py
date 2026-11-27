# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# mk_check_modes2.py (NEW): makes check_modes2.py from check_modes.py (unchanged): caches each Python Merkle root once per (k, alg),
# and also checks the software logs (modes_bench swkt / swflat / swmerkle, xkcp_modes_bench xkcpmerkle) against the same Python references.
src = open(r'C:\Users\elhad\ruty\board\check_modes.py', encoding='utf-8').read()
old = """            H = {'minishake128': mini128, 'turboshake128': turbo128, 'shake128': shake128}[d['alg']]
            lv = [H(vals[k][32 * i:32 * i + 32]) for i in range(1 << k)]
            while len(lv) > 1: lv = [H(lv[2 * i] + lv[2 * i + 1]) for i in range(len(lv) // 2)]
            chk('%s Merkle k=%d %s root vs Python (%s)' % (fn, k, d['mode'], 'pure-Python Keccak-p[800]' if d['alg'] == 'minishake128' else 'pycryptodome' if d['alg'] == 'turboshake128' else 'hashlib'), d['sw_root'], lv[0].hex())"""
new = """            chk('%s Merkle k=%d %s root vs Python (%s)' % (fn, k, d['mode'], 'pure-Python Keccak-p[800]' if d['alg'] == 'minishake128' else 'pycryptodome' if d['alg'] == 'turboshake128' else 'hashlib'), d['sw_root'], proot(k, d['alg']))
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
            chk('%s A53 flat %s len=%d vs Python' % (fn, d['alg'], fl), d['digest'], (shake128 if d['alg'] == 'shake128' else turbo128)(stream[:fl]).hex())"""
assert old in src
src = src.replace(old, new)
helper = """stream = None; vals = {}; roots = {}
def proot(k, alg):
    if (k, alg) not in roots:
        H = {'minishake128': mini128, 'turboshake128': turbo128, 'shake128': shake128}[alg]
        lv = [H(vals[k][32 * i:32 * i + 32]) for i in range(1 << k)]
        while len(lv) > 1: lv = [H(lv[2 * i] + lv[2 * i + 1]) for i in range(len(lv) // 2)]
        roots[(k, alg)] = lv[0].hex()
    return roots[(k, alg)]
"""
assert "stream = None; vals = {}\n" in src
src = src.replace("stream = None; vals = {}\n", helper).replace("# check_modes.py (NEW)", "# check_modes2.py (NEW, made by mk_check_modes2.py from check_modes.py)")
open(r'C:\Users\elhad\ruty\board\check_modes2.py', 'w', encoding='utf-8').write(src)
print('written check_modes2.py')
