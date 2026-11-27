#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""Table of every gem5 calibration setting against the board (real Cortex-A53, no flush). Usage: python3 calib_table.py [md]"""
import glob, os, re, sys
R = os.path.dirname(os.path.abspath(__file__))
NAMES = ['fill_secrets_280', 'fill_chains_280', 'fill_compress_8', 'fill_level_4', 'collect_280', 'tags_copy_240kB',
         'tags_collect_38kB', 'firmware_copy_1MB', 'poll_1000_reads']
SHORT = ['secr', 'chain', 'compr', 'lvl4', 'coll', 'tagcp', 'tagcol', 'fwcp', 'poll']
# processor time of each routine in one real job (22 trees + 8 tag nodes + 2 firmware nodes, with copy): calls per job
CALLS = {'fill_secrets_280': 22, 'fill_chains_280': 22, 'fill_compress_8': 22, 'fill_level_4': 66, 'collect_280': 44,
         'tags_copy_240kB': 8, 'tags_collect_38kB': 8, 'firmware_copy_1MB': 2, 'poll_1000_reads': 20}
def load(path):
    d = {}; ok = False
    for line in open(path, errors='replace'):
        m = re.search(r'COST (\w+) min_ns=(\d+) mean_ns=(\d+)', line)
        if m: d[m.group(1)] = (int(m.group(2)), int(m.group(3)))
        if 'COST end' in line: ok = True
    return d if ok else None
board = load(os.path.join(R, '..', 'results', 'cost_native_a53_noflush_aup1.txt'))
w = {n: CALLS[n] * board[n][1] for n in NAMES}; wt = sum(w.values())
rows = []
for f in sorted(glob.glob(os.path.join(R, 'calib', '*.txt'))):
    n = os.path.basename(f)[:-4]; d = load(f)
    if d is None or any(k not in d for k in NAMES): rows.append((n, None)); continue
    r = [d[k][1] / board[k][1] for k in NAMES]; e = [abs(x - 1) for x in r]
    wm = sum(w[k] * abs(d[k][1] / board[k][1] - 1) for k in NAMES) / wt
    rows.append((n, dict(r=r, mean=sum(e) / len(e), worst=max(e), wmean=wm, d=d)))
md = len(sys.argv) > 1 and sys.argv[1] == 'md'
if md:
    print('| setting | ' + ' | '.join(SHORT) + ' | mean abs err | worst | time-weighted |'); print('|' + '---|' * (len(SHORT) + 4))
else:
    print('%-22s' % 'weights %' + ''.join('%7.1f' % (100 * w[k] / wt) for k in NAMES))
    print('%-22s' % 'setting' + ''.join('%7s' % s for s in SHORT) + '   mean  worst  wmean')
for n, x in sorted(rows, key=lambda t: (t[1] is None, t[1]['mean'] if t[1] else 0)):
    if x is None: print(('| %s | not finished or failed |' if md else '%-22s FAILED') % n); continue
    if md: print('| %s | ' % n + ' | '.join('%.2f' % v for v in x['r']) + ' | %.1f%% | %.1f%% | %.1f%% |' % (100 * x['mean'], 100 * x['worst'], 100 * x['wmean']))
    else: print('%-22s' % n + ''.join('%7.2f' % v for v in x['r']) + ' %6.1f %6.1f %6.1f' % (100 * x['mean'], 100 * x['worst'], 100 * x['wmean']))
if len(sys.argv) > 2:
    x = dict(rows)[sys.argv[2]]
    for k, v in zip(NAMES, x['r']): print('| %s | %d | %d | %.2f |' % (k, board[k][1], x['d'][k][1], v))
