#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""dfg_sim with processor costs taken from the frozen gem5 model instead of fitted.  NOTHING is fitted in this file.

Every processor cost = (gem5 time of the same routine the board runs, board/ruty_cost.rs, no flush)
                     + (cache-flush cost measured on the board; gem5 cannot execute the flush instruction in SE mode).
dfg_sim.py is not edited: its simulate() is re-compiled here with four expressions replaced so that
  - the layout of a 280-item node is split into the secrets fill and the chains fill (dfg_sim uses one cost A for both),
  - small nodes use the compress-8 / level-4 / collect-8 routines instead of one cost S,
  - a stream node costs copy * zero_copy + flush (zero_copy = 0: the scheduler only flushes; dfg_sim scales the whole cost).
Usage: python3 dfg_sim_gem5.py [gem5|board] [md]
"""
import inspect, json, os, sys
import dfg_sim
from dfg_sim import MEAS, P

HERE = os.path.dirname(os.path.abspath(__file__))
C = json.load(open(os.path.join(HERE, 'gem5_costs.json')))

_src = inspect.getsource(dfg_sim.simulate)
for old, new in [("{0: p.A, 1: p.A, '01': p.A}.get(nd['st'], p.S)", "p.fill_tree(nd['st'])"),
                 ("{0: p.B, 1: p.B, '01': p.B}.get(nd['st'], p.S)", "p.col_tree(nd['st'])"),
                 ("p.Tf * zero_copy / tag_split", "p.tag_fill(zero_copy) / tag_split"),
                 ("p.F * zero_copy", "p.fw_fill(zero_copy)")]:
    assert _src.count(old) == 1, old
    _src = _src.replace(old, new)
_ns = dict(vars(dfg_sim)); exec(compile(_src, 'dfg_sim.simulate[gem5 costs]', 'exec'), _ns)
simulate = _ns['simulate']

class PG(P):
    """processor costs in seconds, from a table of routine times (ns) and board flush costs (ns)"""
    def __init__(s, src='gem5', flush='routine'):
        P.__init__(s); r = C[src]; s.src = src
        fl = C['flush_board_routine'] if flush == 'routine' else C['flush_board_streams_only']
        g = lambda k: (r[k] + fl.get(k, 0.0)) * 1e-9
        s.fs, s.fc, s.f8, s.f4 = g('fill_secrets_280'), g('fill_chains_280'), g('fill_compress_8'), g('fill_level_4')
        s.c280, s.c8 = g('collect_280'), g('collect_8')
        s.tcopy, s.tflush = r['tags_copy_240kB'] * 1e-9, C['flush_240kB_ns'] * 1e-9
        s.fcopy, s.fflush = r['firmware_copy_1MB'] * 1e-9, C['flush_1MB_ns'] * 1e-9
        s.Tf = s.tcopy + s.tflush; s.F = s.fcopy + s.fflush
        s.tc_ratio = g('tags_collect_38kB') / s.Tf          # dfg_sim charges Tf * tc_ratio for the tag collect
        s.A, s.B, s.S = (s.fs + s.fc) / 2, s.c280, s.f8     # only for display; simulate() below does not read A or S
    def fill_tree(s, st): return {0: s.fs, 1: s.fc, '01': s.fs, 2: s.f8}.get(st, s.f4)
    def col_tree(s, st): return s.c280 if st in (0, 1, '01') else s.c8
    def tag_fill(s, z): return s.tcopy * z + s.tflush
    def fw_fill(s, z): return s.fcopy * z + s.fflush

NOCOPY = {  # board, graph+rule, no copy, ms (PAPER_FACTS, RQ4): (atoms, MHz, tags, fw)
    (4, 400, 4, 1): 2.802, (4, 400, 8, 2): 3.354, (4, 400, 16, 4): 5.053, (4, 400, 16, 0): 3.564, (4, 400, 0, 4): 3.154,
    (2, 420, 4, 1): 4.850, (2, 420, 8, 2): 5.720, (2, 420, 16, 4): 7.437, (2, 420, 16, 0): 5.970, (2, 420, 0, 4): 5.362}

def run36(p, sim):
    return {k: [sim(k[0], k[1], pol, k[2], k[3], p)['ms'] for pol in range(3)] for k in MEAS}
def run10(p, sim, z):
    return {k: sim(k[0], k[1], 2, k[2], k[3], p, zero_copy=z)['ms'] for k in NOCOPY}
def stats(e): e = [abs(x) for x in e]; return 100 * sum(e) / len(e), 100 * max(e)
def e36(r): return [(r[k][i] - MEAS[k][i]) / MEAS[k][i] for k in MEAS for i in range(3)]
def e10(r): return [(r[k] - NOCOPY[k]) / NOCOPY[k] for k in NOCOPY]

def fitted():
    f = json.load(open(os.path.join(HERE, 'fit.json'))); p = P(**f)
    # the fitted model has no flush term; its no-copy stream cost is the measured flush as a fraction of its fitted copy
    zt, zf = C['flush_240kB_ns'] * 1e-9 / p.Tf, C['flush_1MB_ns'] * 1e-9 / p.F
    class PF(P):
        def fill_tree(s, st): return {0: s.A, 1: s.A, '01': s.A}.get(st, s.S)
        def col_tree(s, st): return {0: s.B, 1: s.B, '01': s.B}.get(st, s.S)
        def tag_fill(s, z): return s.Tf if z == 1.0 else s.Tf * zt
        def fw_fill(s, z): return s.F if z == 1.0 else s.F * zf
    return PF(**f)

if __name__ == '__main__':
    out = []
    models = [('fitted (5 costs fitted to the 36 runs)', fitted()),
              ('gem5 + board flush, no fitting', PG('gem5', 'routine')),
              ('gem5 + flush on streams only', PG('gem5', 'streams')),
              ('board routine times + board flush (ceiling for any processor model)', PG('board', 'routine'))]
    res = {}
    for name, p in models:
        r36 = run36(p, simulate); r10 = run10(p, simulate, 0.0)
        e = e36(r36); m36, w36 = stats(e); m10, w10 = stats(e10(r10)); bias36 = 100 * sum(e) / len(e); bias10 = 100 * sum(e10(r10)) / 10
        res[name] = (r36, r10)
        print('%-70s 36 runs: mean %.1f%% worst %.1f%% bias %+.1f%% | 10 no-copy: mean %.1f%% worst %.1f%% bias %+.1f%%' % (name, m36, w36, bias36, m10, w10, bias10))
    for name, p in models[1:2]:
        print('costs used, us: secrets %.2f chains %.2f compress8 %.2f level4 %.2f collect280 %.2f collect8 %.2f | tags copy %.1f + flush %.1f, collect %.1f | fw copy %.1f + flush %.1f'
              % tuple(x * 1e6 for x in (p.fs, p.fc, p.f8, p.f4, p.c280, p.c8, p.tcopy, p.tflush, p.Tf * p.tc_ratio, p.fcopy, p.fflush)))
    names = [m[0] for m in models]
    print('\n36 calibration runs (with copy), ms: board | fitted | gem5-based ; serial graph graph+rule')
    for k in MEAS:
        print('  atoms=%d tags=%2d fw=%d  %s | %s | %s' % (k[0], k[2], k[3], ' '.join('%6.3f' % x for x in MEAS[k]),
              ' '.join('%6.3f' % x for x in res[names[0]][0][k]), ' '.join('%6.3f' % x for x in res[names[1]][0][k])))
    print('\n10 no-copy rows (graph+rule), ms: board | fitted | gem5-based (error)')
    for k in NOCOPY:
        a, b = res[names[0]][1][k], res[names[1]][1][k]
        print('  atoms=%d tags=%2d fw=%d  %6.3f | %6.3f (%+5.1f%%) | %6.3f (%+5.1f%%)' % (k[0], k[2], k[3], NOCOPY[k], a, 100 * (a / NOCOPY[k] - 1), b, 100 * (b / NOCOPY[k] - 1)))
