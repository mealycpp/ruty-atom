#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""Rank the thirteen scheduler designs on gem5 processor costs, with nothing fitted.

The designs are the same thirteen options as design_ranking.txt (which used processor costs fitted to 36 board runs).
Here every processor cost comes from the frozen gem5 model (gem5_costs.json) plus the cache-flush cost measured on the board,
as in dfg_sim_gem5.py; the atoms and the memory port keep their board timings. Four atoms at 400 MHz, graph+rule policy.
With the argument 'fitted' the script first reproduces design_ranking.txt from fit.json, as a check of the option table.

Run: python3 rank13_gem5.py            (gem5 costs)
     python3 rank13_gem5.py fitted     (reproduces design_ranking.txt)
"""
import json, os, sys
import dfg_sim, dfg_sim_gem5 as G
HERE = os.path.dirname(os.path.abspath(__file__))
OPT = [
 ('chains cut in 4 + one node',               dict(chain_split=4, merge01=True),                 'keeps copy'),
 ('chains cut in 4 + one node + no copy',     dict(chain_split=4, merge01=True, zero_copy=0.0),  'removes copy'),
 ('chains cut in 4 + one node + second core', dict(chain_split=4, merge01=True, helper=True),    'removes copy'),
 ('tree chains cut in 2 nodes',               dict(chain_split=2),                               'keeps copy'),
 ('tree chains cut in 4 nodes',               dict(chain_split=4),                               'keeps copy'),
 ('tree chains cut in 8 nodes',               dict(chain_split=8),                               'keeps copy'),
 ('secrets+chains as one hardware node',      dict(merge01=True),                                'keeps copy'),
 ('no copy for stream data',                  dict(zero_copy=0.0),                               'removes copy'),
 ('port budget 1',                            dict(budget=1),                                    'keeps copy'),
 ('port budget 3',                            dict(budget=3),                                    'keeps copy'),
 ('port budget 4',                            dict(budget=4),                                    'keeps copy'),
 ('second core lays out stream data',         dict(helper=True),                                 'removes copy'),
 ('tag nodes cut in 4',                       dict(tag_split=4),                                 'keeps copy'),
]
MIXES = [(0, 0), (4, 1), (8, 2), (16, 4), (16, 0), (0, 4)]          # tag nodes, firmware nodes; the last five are the mixed jobs run on the board
BOARD = {(4, 1): (4.039, 2.802), (8, 2): (5.792, 3.354), (16, 4): (8.877, 5.053), (16, 0): (6.108, 3.564), (0, 4): (5.208, 3.154)}  # with copy, no copy (same run), ms

def main(fitted):
    if fitted:
        p = dfg_sim.P()
        for k, v in json.load(open(os.path.join(HERE, 'fit.json'))).items(): setattr(p, k, v)
        sim = dfg_sim.simulate; print('Processor costs: fitted to 36 board runs (fit.json). 4 atoms @400 MHz, graph+rule.')
    else:
        p = G.PG('gem5', 'routine'); sim = G.simulate
        print('Processor costs: gem5 (frozen model) + board cache flush, nothing fitted. 4 atoms @400 MHz, graph+rule.')
    keep, rem = [], {}
    for tb, fw in MIXES:
        base = sim(4, 400, 2, tb, fw, p)['ms']
        rows = sorted((sim(4, 400, 2, tb, fw, p, **kw)['ms'], n, c) for n, kw, c in OPT)
        print(' mix tags=%d fw=%d: baseline (with copy) %.3f ms' % (tb, fw, base))
        for ms, n, c in rows:
            ch = 100 * (ms / base - 1); print('    %-42s %.3f (%+.1f%%)  %s' % (n, ms, ch, c))
            if (tb, fw) != (0, 0):
                if c == 'keeps copy': keep.append(ch)
                else: rem.setdefault(n, []).append(ch)
    print('\nMixed jobs (five mixes), change in job time against the baseline:')
    print('  nine designs that keep the copy: %+.1f%% to %+.1f%%' % (min(keep), max(keep)))
    for n, v in rem.items(): print('  %-42s %+.1f%% to %+.1f%%' % (n, min(v), max(v)))
    if not fitted:
        print('\nNo copy, model against board (ms; saving against with-copy of the same source):')
        for k, (bc, bn) in BOARD.items():
            mc = sim(4, 400, 2, k[0], k[1], p)['ms']; mn = sim(4, 400, 2, k[0], k[1], p, zero_copy=0.0)['ms']
            print('  tags=%2d fw=%d  model %.3f -> %.3f (-%.1f%%)   board %.3f -> %.3f (-%.1f%%)   no-copy time error %+.1f%%'
                  % (k[0], k[1], mc, mn, 100 * (1 - mn / mc), bc, bn, 100 * (1 - bn / bc), 100 * (mn / bn - 1)))
        print('\nAtom count (model only beyond four), 400 MHz, ms: trees only, today\'s design / mixed (8 tags, 2 firmware), no copy')
        for n in (1, 2, 4, 6, 8, 12, 16):
            print('  atoms=%2d  %.3f / %.3f' % (n, sim(n, 400, 2, 0, 0, p)['ms'], sim(n, 400, 2, 8, 2, p, zero_copy=0.0)['ms']))
if __name__ == '__main__':
    main(len(sys.argv) > 1 and sys.argv[1] == 'fitted')
