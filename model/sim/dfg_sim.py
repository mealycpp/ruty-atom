#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""Event simulator of the RUTY job-graph scheduler (one processor core, N atoms, one shared memory port).

It replays the scheduling loop of board/ruty_dfg.rs step by step:
  the processor walks over the atoms; a finished node is collected (processor time), a free atom gets the next node the policy
  picks, the node's data is laid out (processor time), the atom then runs it (hardware time). The processor does one thing at a time.
Stream nodes (tags, firmware) draw on the memory port; when their demands exceed the port, all of them slow down in proportion.

Costs:
  hardware times come from board measurements made before this model existed (chain step, tags per second, firmware GB/s);
  processor times per node kind are the free parameters; they are fitted to the 36 board runs of 2026-11-26 (calibration set).
"""
import sys, itertools, json

NTREE, NCH = 22, 280
PORT = 2.4e9                      # bytes per second, measured port limit (64-bit word per 300 MHz clock)
TAG_BYTES, FW_BYTES = 240_000, 1 << 20
TAG_DEMAND, FW_DEMAND = 1.31e9, 2.2e9   # bytes/s one atom draws when alone (13.1 M tags/s x 100 B; 1 MB firmware at 2.2 GB/s)

def hw_tree(stage, mhz):
    """hardware time of one tree node in seconds"""
    clk = 1.0 / (mhz * 1e6)
    if stage == 0: return NCH * 38 * clk                 # 280 one-block SHAKE256 hashes: 24 rounds + about 14 clocks in/out
    if stage == 1: return NCH * 15 * 26.9 * clk          # 280 chains of 15 steps, 26.9 clocks per step (measured)
    if stage == 2: return 8 * 200 * clk                  # 8 hashes of 608 bytes (5 blocks)
    return (8 >> (stage - 2)) * 60 * clk                 # 4, 2, 1 hashes of 80 bytes

class P:  # processor costs in seconds; defaults are replaced by the fit
    def __init__(s, A=18e-6, B=14e-6, S=2e-6, Tf=200e-6, F=700e-6, poll=0.12e-6, tc_ratio=0.16):
        s.A, s.B, s.S, s.Tf, s.F, s.poll, s.tc_ratio = A, B, S, Tf, F, poll, tc_ratio

def simulate(n, mhz, policy, tb, fwn, p, budget=2, merge01=False, helper=False, zero_copy=1.0, tag_split=1, chain_split=1, trace=False):
    """returns makespan (s), per-tenant finish times, atom utilization.
    merge01: secrets and chains run as one hardware node (no return to the processor between them)
    helper: a second processor core lays out stream data ahead of time (the scheduling core only starts the node)
    zero_copy: factor on the stream layout cost (0 = the tenant wrote its data straight into the atom buffer)
    tag_split: each tag node is cut into this many smaller nodes"""
    # nodes: (kind, a, stage, succ, units)
    nodes = []; ks = chain_split; rt = []
    for t in range(NTREE):                                  # per tree: ks parallel parts of (secrets -> chains), then compression and 3 levels
        base = len(nodes); head = ['01'] if merge01 else [0, 1]; join = base + ks * len(head)
        for j in range(ks):
            for i, st in enumerate(head):
                me = base + j * len(head) + i
                nodes.append(dict(kind='tree', a=t, st=st, pr=i, succ=(me + 1 if i < len(head) - 1 else join), units=0, frac=1.0 / ks, ndep=0))
            rt.append(base + j * len(head))
        for i, st in enumerate([2, 3, 4, 5]):
            nodes.append(dict(kind='tree', a=t, st=st, pr=2 + i, succ=(join + i + 1 if i < 3 else None), units=0, frac=1.0, ndep=(ks if i == 0 else 1)))
    ntree_nodes = len(nodes)
    for k in range(tb * tag_split): nodes.append(dict(kind='tags', a=k, st=0, pr=0, succ=None, units=1, bytes=TAG_BYTES / tag_split, frac=1.0, ndep=0))
    for k in range(fwn): nodes.append(dict(kind='fw', a=k, st=0, pr=0, succ=None, units=2, bytes=FW_BYTES, frac=1.0, ndep=0))
    rs = list(range(ntree_nodes, len(nodes)))
    at = [None] * n; fin = [None] * n; since = [0.0] * n; busy = 0.0
    left = len(nodes); trees_left = ntree_nodes; tags_left = tb * tag_split; fw_left = fwn
    tt = tg = tf = 0.0; units_run = 0; flip = False; t = 0.0
    streams = {}                                     # atom -> [remaining bytes, demand]
    last = [0.0]
    def advance(to):                                 # move stream work forward to time `to` with proportional port sharing
        while last[0] < to - 1e-15:
            if not streams: last[0] = to; return
            tot = sum(d for _, d in streams.values()); scale = min(1.0, PORT / tot)
            dt_done = min(r / (d * scale) for r, d in streams.values())
            step = min(to - last[0], dt_done)
            for a in list(streams):
                r, d = streams[a]; r -= d * scale * step
                if r <= 1e-6: fin[a] = last[0] + step; del streams[a]
                else: streams[a][0] = r
            last[0] += step
    def next_stream_finish():
        if not streams: return None
        tot = sum(d for _, d in streams.values()); scale = min(1.0, PORT / tot)
        return last[0] + min(r / (d * scale) for r, d in streams.values())
    def take_tree():
        if not rt: return None
        bi = max(range(len(rt)), key=lambda i: (nodes[rt[i]]['pr'], -i)); x = rt[bi]; rt[bi] = rt[-1]; rt.pop(); return x
    def take_stream(b):
        for i, x in enumerate(rs):
            if nodes[x]['units'] <= b: return rs.pop(i)
        return None
    def fill_cost(nd):
        if nd['kind'] == 'tree':
            return {0: p.A, 1: p.A, '01': p.A}.get(nd['st'], p.S) * (nd['frac'] if nd['st'] in (0, 1, '01') else 1.0) + (0.5e-6 if nd['frac'] < 1 else 0.0)
        if nd['kind'] == 'tags': return (0.0 if helper else p.Tf * zero_copy / tag_split) + 1e-6
        return (0.0 if helper else p.F * zero_copy) + 1e-6
    def col_cost(nd):
        if nd['kind'] == 'tree': return {0: p.B, 1: p.B, '01': p.B}.get(nd['st'], p.S) * (nd['frac'] if nd['st'] in (0, 1, '01') else 1.0) + (0.5e-6 if nd['frac'] < 1 else 0.0)
        if nd['kind'] == 'tags': return p.Tf * p.tc_ratio / tag_split
        return 1e-6
    guard = 0
    while left > 0:
        guard += 1
        if guard > 5_000_000: raise RuntimeError('no progress')
        did = False
        for ai in range(n):
            t += p.poll; advance(t)
            if at[ai] is not None and fin[ai] is not None and fin[ai] <= t:
                nd = nodes[at[ai]]; busy += t - since[ai]; t += col_cost(nd); advance(t)
                if nd['kind'] == 'tree':
                    trees_left -= 1
                    if trees_left == 0: tt = t
                elif nd['kind'] == 'tags':
                    tags_left -= 1
                    if tags_left == 0: tg = t
                else:
                    fw_left -= 1
                    if fw_left == 0: tf = t
                units_run -= nd['units']
                if nd['succ'] is not None:
                    nodes[nd['succ']]['ndep'] -= 1
                    if nodes[nd['succ']]['ndep'] <= 0: rt.append(nd['succ'])
                left -= 1; at[ai] = None; fin[ai] = None; did = True
            if at[ai] is None and left > 0:
                if policy == 0: pick = take_tree() if trees_left > 0 else take_stream(10**9)
                elif policy == 1:
                    flip = not flip
                    pick = (take_stream(10**9) or take_tree()) if flip else (take_tree() if rt else take_stream(10**9))
                    if pick is None and flip and rt: pick = take_tree()
                else:
                    pick = take_stream(max(0, budget - units_run) if units_run > 0 else 10**9)
                    if pick is None: pick = take_tree()
                if pick is not None:
                    nd = nodes[pick]; t += fill_cost(nd); advance(t); since[ai] = t; at[ai] = pick; units_run += nd['units']; did = True
                    if nd['kind'] == 'tree':
                        h = (hw_tree(0, mhz) + hw_tree(1, mhz)) * nd['frac'] if nd['st'] == '01' else hw_tree(nd['st'], mhz) * (nd['frac'] if nd['st'] in (0, 1) else 1.0); fin[ai] = t + h
                    else:
                        streams[ai] = [nd['bytes'], TAG_DEMAND if nd['kind'] == 'tags' else FW_DEMAND]; fin[ai] = None
        if not did:                                   # nothing to do: the processor spins until the next atom finishes
            cands = [f for a, f in enumerate(fin) if at[a] is not None and f is not None and f > t]
            ns = next_stream_finish()
            if ns is not None: cands.append(ns)
            if cands: t = max(t, min(cands)); advance(t)
    return dict(ms=t * 1e3, trees=tt * 1e3, tags=tg * 1e3, fw=tf * 1e3, util=busy / (n * t))

MEAS = {  # board, 2026-11-26, best of 7: (atoms, MHz, tags, fw) -> [serial, graph, graph+rule] in ms
    (4, 400, 0, 0): [2.325, 2.330, 2.327], (4, 400, 4, 1): [4.386, 3.892, 3.848], (4, 400, 8, 2): [6.116, 5.640, 5.354],
    (4, 400, 16, 4): [9.620, 8.884, 8.858], (4, 400, 16, 0): [6.144, 5.747, 5.781], (4, 400, 0, 4): [5.862, 5.379, 5.239],
    (2, 420, 0, 0): [4.042, 4.095, 4.014], (2, 420, 4, 1): [6.250, 5.981, 5.795], (2, 420, 8, 2): [8.111, 7.412, 7.439],
    (2, 420, 16, 4): [11.095, 10.715, 10.819], (2, 420, 16, 0): [7.715, 7.524, 7.565], (2, 420, 0, 4): [7.562, 7.346, 7.377]}

def err(p, keys=None):
    e = [];
    for k, v in MEAS.items():
        if keys and k not in keys: continue
        for pol in range(3):
            s = simulate(k[0], k[1], pol, k[2], k[3], p)['ms']; e.append((s - v[pol]) / v[pol])
    return e

def fit(keys=None):
    p = P(); names = ['A', 'B', 'S', 'Tf', 'F']; step = {k: getattr(p, k) * 0.3 for k in names}
    best = sum(x * x for x in err(p, keys))
    for _ in range(60):
        improved = False
        for k in names:
            for sgn in (1, -1):
                old = getattr(p, k); new = max(1e-7, old + sgn * step[k]); setattr(p, k, new)
                e = sum(x * x for x in err(p, keys))
                if e < best - 1e-9: best = e; improved = True; break
                setattr(p, k, old)
        if not improved:
            for k in names: step[k] *= 0.5
            if max(step[k] / getattr(p, k) for k in names) < 0.004: break
    return p

if __name__ == '__main__':
    mode = sys.argv[1] if len(sys.argv) > 1 else 'fit'
    if mode == 'fit':
        p = fit(); e = err(p)
        print('fitted processor costs (microseconds): ' + ', '.join('%s=%.1f' % (k, getattr(p, k) * 1e6) for k in ['A', 'B', 'S', 'Tf', 'F']))
        print('calibration: %d runs, mean |error| %.1f%%, worst %.1f%%' % (len(e), 100 * sum(abs(x) for x in e) / len(e), 100 * max(abs(x) for x in e)))
        for k, v in MEAS.items():
            s = [simulate(k[0], k[1], pol, k[2], k[3], p)['ms'] for pol in range(3)]
            print('  atoms=%d tags=%2d fw=%d  board %s  model %s' % (k[0], k[2], k[3], ' '.join('%6.3f' % x for x in v), ' '.join('%6.3f' % x for x in s)))
        json.dump({k: getattr(p, k) for k in ['A', 'B', 'S', 'Tf', 'F', 'poll', 'tc_ratio']}, open('fit.json', 'w'))
