# Event simulator on gem5-derived processor costs (2026-11-26)

`dfg_sim_gem5.py` runs the same scheduler replay as `dfg_sim.py` (imported, not edited) but takes every processor cost from the frozen gem5 model
(`model/gem5_atom/FROZEN_MODEL.md`: HPI core, degree-2 stride prefetcher on L1D, 1-cycle L1, 4-cycle prefetch issue; 14.5% mean, 32.3% worst,
11.2% time-weighted error per routine against the real A53) plus the cache-flush costs measured on the board. **Nothing is fitted.** Inputs are in `gem5_costs.json`.

## Costs used (microseconds)

| processor step | gem5 (no flush) | + board flush | = cost used | fitted cost it replaces |
|---|---|---|---|---|
| layout of 280 secrets | 15.48 | 1.54 | 17.02 | A = 5.01 (one value for both layouts) |
| layout of 280 chains | 13.65 | 2.77 | 16.43 | A = 5.01 |
| collect 280 digests | 1.79 | 1.72 | 3.51 | B = 6.95 |
| small node: layout of 8 (compress) / of 4 and below / collect | 3.16 / 0.42 / 0.13 | 1.93 / 0.20 / 0.28 | 5.09 / 0.62 / 0.41 | S = 1.64 (one value for all) |
| tag node: copy 240 kB, then flush | 124.27 | 28.5 | 152.8 (no-copy: 28.5) | Tf = 189.2 |
| tag node: flush, then collect 38 kB | 9.40 | 22.44 | 31.83 | 0.16 x Tf = 30.3 |
| firmware node: copy 1 MB, then flush | 646.47 | 124.6 | 771.1 (no-copy: 124.6) | F = 726.3 |

Flush on the two stream copies is the board flush-only measurement (28.5 us per 240 kB, 124.6 us per 1 MB). Flush on the other steps is the board time of the routine
with flush minus without flush (`cost_native_a53_aup1.txt` minus `cost_native_a53_noflush_aup1.txt`); for the tag collect this is 22.4 us, far more than the flush of 38 kB alone (4.6 us),
because the collect then reads cold lines. The extension to `simulate()` is four replaced expressions (secrets and chains layouts separate; three small-node routines; stream cost = copy x zero_copy + flush).
Unchanged from `dfg_sim.py` and NOT from gem5: hardware times of the atoms and the port (board), 0.12 us per poll and 1 us per stream-node start (assumed; register access to the device is not in gem5).

## Error against the board, no fitting

| processor costs | 36 runs with copy: mean / worst / bias | 10 no-copy rows: mean / worst / bias |
|---|---|---|
| fitted to the 36 runs (5 parameters; the reference) | 2.9% / 8.3% / 0.0% | 3.8% / 10.1% / -1.7% (*) |
| **gem5 + board flush, no fitting** | **4.9% / 9.5% / +0.4%** | **4.8% / 9.0% / +2.6%** |
| gem5 + flush on stream nodes only (tag-collect flush scaled by bytes, none on tree nodes) | 6.0% / 17.4% / -4.0% | 4.9% / 13.3% / -2.4% |
| board routine times + board flush (no gem5 at all) | 4.2% / 12.9% / +3.1% | 4.6% / 9.9% / +1.2% |

(*) The fitted model has no flush term; for this row its no-copy stream cost is the measured flush. The forecast filed before the run used a 0 to 15% band instead.

No-copy rows, graph+rule, ms (zero_copy = 0, i.e. flush only):

| atoms | tag nodes | fw nodes | board | fitted model | gem5-based model |
|---|---|---|---|---|---|
| 4 | 4 | 1 | 2.802 | 2.763 (-1.4%) | 2.920 (+4.2%) |
| 4 | 8 | 2 | 3.354 | 3.178 (-5.3%) | 3.445 (+2.7%) |
| 4 | 16 | 4 | 5.053 | 4.542 (-10.1%) | 4.598 (-9.0%) |
| 4 | 16 | 0 | 3.564 | 3.361 (-5.7%) | 3.483 (-2.3%) |
| 4 | 0 | 4 | 3.154 | 3.052 (-3.2%) | 3.151 (-0.1%) |
| 2 | 4 | 1 | 4.850 | 5.065 (+4.4%) | 5.144 (+6.1%) |
| 2 | 8 | 2 | 5.720 | 5.845 (+2.2%) | 6.236 (+9.0%) |
| 2 | 16 | 4 | 7.437 | 7.721 (+3.8%) | 8.105 (+9.0%) |
| 2 | 16 | 0 | 5.970 | 5.889 (-1.3%) | 6.103 (+2.2%) |
| 2 | 0 | 4 | 5.362 | 5.349 (-0.2%) | 5.563 (+3.8%) |

Per-run numbers for the 36: `python3 dfg_sim_gem5.py`. Largest misses of the gem5-based model there: 4 atoms, 16 tags, no firmware, serial 5.562 against 6.144 (-9.5%);
4 atoms, firmware only, graph+rule 5.713 against 5.239 (+9.0%); 2 atoms, 8 tags + 2 firmware, graph+rule 8.111 against 7.439 (+9.0%). Trees only: 2.502 against 2.327 at 4 atoms (+7.5%), 4.012 against 4.014 at 2 atoms.

## What this does and does not show

- The gem5-based model is about 2 points worse than the fitted one on the 36 runs (4.9% against 2.9% mean, 9.5% against 8.3% worst) and equal within a point on the 10 no-copy rows, which neither model saw.
  It has zero free parameters; the fitted one has five, and its fitted values (A = 5.0 us, B = 6.9 us) do not match what the routines cost on the board (13 to 14 us and 1.6 us): the fit trades them against each other.
- The processor model is not what limits the simulator. With the board's own routine times in place of gem5's (last row) the error is the same size (4.2% / 12.9%).
  gem5's per-routine errors of up to 32% fall on routines that carry 1 to 2% of the time, and its 21% fast tag copy partly cancels a slow bias of the simulator's structure; that cancellation is luck, not accuracy, and must not be sold as accuracy.
- The flush accounting matters more than the processor model: charging the flush only on the stream copies, as bytes, raises the worst case from 9.5% to 17.4%.
- Resolution: about 5% on average and 10% worst case. The board itself moves by that much between runs of the same case (graph+rule with copy, 4 atoms, 8 tags + 2 firmware: 5.354 ms in the calibration run, 5.792 ms in the no-copy session).
  The model therefore supports decisions worth more than 10% (no copy: 31 to 43% measured at 4 atoms) and does not support ordering options that differ by less.
  It picks the board's best of the three policies in 4 of the 5 mixed cases at four atoms and in 0 of 5 at two atoms (the fitted model: 4 of 5 and 0 of 5). At two atoms both models put serial first in four cases,
  while on the board graph or graph+rule wins by 2 to 9%; this is an error of the simulator's structure (same with fitted costs), not of the processor costs.
- Not tested: a forecast made with the gem5-based model BEFORE a measurement. Both data sets above were measured before this model existed, although no number in the model was taken from them.

## Three sentences for the paper

1. The processor costs of the event simulator are not fitted: each is the time of the scheduler's own routine in a gem5 model of the Cortex-A53 (ARM's HPI in-order core with a degree-2 stride prefetcher, calibrated on nine routines to 14.5% mean and 32% worst error against the board), plus the cache-flush cost measured on the board, which gem5's system-call emulation mode does not execute.
2. With no parameter fitted to scheduler runs, the simulator reproduces the 36 with-copy board runs with 4.9% mean and 9.5% worst error and the ten no-copy runs with 4.8% mean and 9.0% worst error, against 2.9% and 8.3% on the 36 runs for a five-parameter fit.
3. This resolves design choices worth more than about 10% of the job time, such as removing the copy (31 to 43% at four atoms), and it does not resolve the choice among scheduling policies that lie within 10% of each other.

Verdict: **good enough to carry the forecasts**, at a stated resolution of 10%, and only with the routine-level flush costs from the board. It is not good enough to rank the three policies at two atoms (it gets all five cases wrong there, as does the fitted version).
