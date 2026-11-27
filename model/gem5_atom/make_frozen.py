#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""Writes FROZEN_MODEL.md from calib/*.txt, the grid scripts and the board file. Run: python3 make_frozen.py"""
import glob, os, re
R = os.path.dirname(os.path.abspath(__file__)); FROZEN = 'r4_pflat4'
NAMES = ['fill_secrets_280', 'fill_chains_280', 'fill_compress_8', 'fill_level_4', 'collect_280', 'tags_copy_240kB', 'tags_collect_38kB', 'firmware_copy_1MB', 'poll_1000_reads']
SHORT = ['secrets', 'chains', 'compr8', 'level4', 'coll280', 'tag copy', 'tag coll', 'fw copy', 'poll']
CALLS = dict(zip(NAMES, [22, 22, 22, 66, 44, 8, 8, 2, 20]))
def load(path):
    d = {}; ok = False
    for line in open(path, errors='replace'):
        m = re.search(r'COST (\w+) min_ns=(\d+) mean_ns=(\d+)', line)
        if m: d[m.group(1)] = int(m.group(3))
        ok |= 'COST end' in line
    return d if ok else None
board = load(os.path.join(R, '..', 'results', 'cost_native_a53_noflush_aup1.txt'))
w = {n: CALLS[n] * board[n] for n in NAMES}; wt = sum(w.values())
opts = {}; order = []
for i, g in enumerate(['calib_grid.sh', 'calib_grid2.sh', 'calib_grid3.sh', 'calib_grid4.sh', 'calib_grid5.sh']):
    for line in open(os.path.join(R, g)):
        m = re.match(r'run (\S+)\s*(.*?)\s*&', line)
        if m: opts[m.group(1)] = (i + 1, ' '.join(m.group(2).replace('$B', '--cpu hpi --l1pf 2 --l1lat 1').split()) or '(stock: in-order Minor core, no prefetcher)'); order.append(m.group(1))
rows = {}
for n in order:
    d = load(os.path.join(R, 'calib', n + '.txt')); r = [d[k] / board[k] for k in NAMES]; e = [abs(x - 1) for x in r]
    rows[n] = dict(d=d, r=r, mean=100 * sum(e) / 9, worst=100 * max(e), wm=100 * sum(w[k] * abs(d[k] / board[k] - 1) for k in NAMES) / wt)
f = rows[FROZEN]; o = []
o.append('# Frozen gem5 processor model for the RUTY atom paper (frozen 2026-11-26)\n')
o.append('What it is: a gem5 (v26, SE mode, existing `build/ARM/gem5.opt`, not rebuilt) model of ONE Cortex-A53 core of the AUP-ZU3 at 1.2 GHz. It models the processor only.')
o.append('The atoms and the memory port are measured on the board, not modelled here. The same static binary (`ruty_cost_aarch64`, built from board/ruty_cost.rs) runs on the board and in gem5.\n')
o.append('## Frozen setting\n')
o.append('ARM\'s HPI in-order core (gem5 `configs/common/cores/arm/HPI.py`), 32 kB 4-way L1D with 1-cycle tag/data latency and 8 miss buffers, 1 MB 16-way L2 at 12 cycles, DDR4-2400 8x8,')
o.append('stride prefetcher on L1D only: degree 2, 4 cycles before a generated prefetch may issue. Everything else is the default of `ruty_se2.py`.\n')
o.append('```\ncd ~/gem5\nbuild/ARM/gem5.opt -d /mnt/c/Users/elhad/ruty/gem5_atom/calib/m5out_r4_pflat4 /mnt/c/Users/elhad/ruty/gem5_atom/ruty_se2.py \\\n    --cmd /mnt/c/Users/elhad/ruty/gem5_atom/ruty_cost_aarch64 --options "0 3" --cpu hpi --l1pf 2 --l1lat 1 --pflat 4\n```')
o.append('Output kept in `calib/r4_pflat4.txt`; full gem5 configuration in `calib/m5out_r4_pflat4/config.ini`. gem5 is deterministic: the same command gives the same numbers.\n')
o.append('## Result against the board (no flush; board = mean of 40 repetitions on aup1, gem5 = mean of 3)\n')
o.append('| routine | board A53 ns | gem5 ns | gem5 / board | share of processor time in a job |\n|---|---|---|---|---|')
for k, v in zip(NAMES, f['r']): o.append('| %s | %d | %d | %.2f | %.1f%% |' % (k, board[k], f['d'][k], v, 100 * w[k] / wt))
o.append('\nMean absolute error over the nine routines: **%.1f%%**. Worst: **%.1f%%** (poll_1000_reads). Weighted by processor time in a job: **%.1f%%**.' % (f['mean'], f['worst'], f['wm']))
o.append('The weights are the calls of each routine in one job of 22 trees + 8 tag nodes + 2 firmware nodes times the board time per call; the two copies carry 75% of the time.\n')
o.append('**The target (mean at or below 10%, worst at or below 20%) was NOT met.** The search was stopped at the agreed limit of 40 settings instead of adding more knobs.')
o.append('The error has a clear shape: the two memory copies come out 2% and 21% faster than the board, the compute routines 5% to 32% slower.')
o.append('No setting tried brings the tag copy and the 1 MB copy to the board together: with 8 L1D miss buffers they are 0.79 and 0.98; with 4 (the figure in ARM\'s own HPI cache set) 1.02 and 1.77.')
o.append('The poll loop (4.1 cycles per iteration in gem5, 3.1 on the board) and the small-node layouts did not respond to any cache or prefetcher knob; they are set by the core model itself, which was not touched.\n')
o.append('How the frozen setting was chosen: lowest time-weighted error, and it also has the lowest plain mean. Six more settings lie within 1.4 points of it on both measures (rows 2 to 7 below);')
o.append('the round-3 best (`--cpu hpi --l1pf 2 --l1lat 1`, 15.1% / 31.8% / 12.6%) differs from the frozen one by at most 2.1% on any routine. The choice among them does not change any conclusion.\n')
o.append('## Every setting tried (40, five rounds of 8), best plain mean first\n')
o.append('Columns are gem5 / board per routine. All runs used `--options "0 3"`, 1.2 GHz unless stated. Nothing is left out; `calib/<name>.txt` holds each raw output.\n')
o.append('| # | round | name | options | ' + ' | '.join(SHORT) + ' | mean | worst | time-weighted |\n|' + '---|' * (len(SHORT) + 7))
for i, n in enumerate(sorted(order, key=lambda n: rows[n]['mean'])):
    x = rows[n]; o.append('| %d | %d | %s%s | `%s` | ' % (i + 1, opts[n][0], n, ' **(frozen)**' if n == FROZEN else '', opts[n][1]) + ' | '.join('%.2f' % v for v in x['r']) + ' | %.1f%% | %.1f%% | %.1f%% |' % (x['mean'], x['worst'], x['wm']))
o.append('\nNotes on the list: `r4_pfmiss` (prefetcher trained on misses only) and `r3_hpi_pf2_wb16` (16 write buffers) reproduce their parent setting to the digit, so those two knobs do nothing here.')
o.append('The out-of-order core (`o3_pf8`) is 2.6 times too fast and was dropped after round 1. Every setting with an L2 prefetcher makes the copies too fast (13% to 57%).\n')
o.append('## What this model cannot give\n')
o.append('1. **The cache-flush instruction.** In SE mode gem5 does not execute the data-cache flush (the two `*_flush_only` rows come out at about 140 ns, i.e. nothing). The binary is therefore always run with flush = 0 in gem5,')
o.append('   and the flush cost stays a board measurement: 28.5 us per 240 kB and 124.6 us per 1 MB (13% to 15% of the copy), plus what the flush adds to the routine that next reads the flushed lines')
o.append('   (tag collect: 8.9 us without flush, 31.3 us with flush on the board).')
o.append('2. **Register access to the real device.** `poll_1000_reads` and `shape_5_writes` touch ordinary memory in this binary, on the board and in gem5. The latency of a read or write that crosses the bus to an atom\'s registers is not in the model.')
o.append('3. **The rest of the system.** One core, no operating system, no interrupts, no other core sharing the L2; the board numbers are means under Linux and include that noise (board mean is 1% to 64% above board minimum, by routine).')
o.append('4. **Repetitions.** gem5 runs 3 repetitions (the first one cold) against 40 on the board; for the tag copy the gem5 minimum is 8% below its mean.')
o.append('5. **The atoms and the memory port.** Not modelled; measured.\n')
o.append('Files: `ruty_se2.py` (model; round-4 options added with defaults that reproduce rounds 1 to 3), `calib_grid.sh` to `calib_grid5.sh` (the five rounds), `calib_table.py` (text table), `make_frozen.py` (writes this file), `calib/` (raw outputs and gem5 output directories).')
open(os.path.join(R, 'FROZEN_MODEL.md'), 'w', newline='\n').write('\n'.join(o) + '\n'); print('written', len(o), 'lines; frozen', FROZEN, '%.1f %.1f %.1f' % (f['mean'], f['worst'], f['wm']))
