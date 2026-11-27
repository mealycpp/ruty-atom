# Frozen gem5 processor model for the RUTY atom paper (frozen 2026-11-26)

What it is: a gem5 (v26, SE mode, existing `build/ARM/gem5.opt`, not rebuilt) model of ONE Cortex-A53 core of the AUP-ZU3 at 1.2 GHz. It models the processor only.
The atoms and the memory port are measured on the board, not modelled here. The same static binary (`ruty_cost_aarch64`, built from board/ruty_cost.rs) runs on the board and in gem5.

## Frozen setting

ARM's HPI in-order core (gem5 `configs/common/cores/arm/HPI.py`), 32 kB 4-way L1D with 1-cycle tag/data latency and 8 miss buffers, 1 MB 16-way L2 at 12 cycles, DDR4-2400 8x8,
stride prefetcher on L1D only: degree 2, 4 cycles before a generated prefetch may issue. Everything else is the default of `ruty_se2.py`.

```
cd ~/gem5
build/ARM/gem5.opt -d /mnt/c/Users/elhad/ruty/gem5_atom/calib/m5out_r4_pflat4 /mnt/c/Users/elhad/ruty/gem5_atom/ruty_se2.py \
    --cmd /mnt/c/Users/elhad/ruty/gem5_atom/ruty_cost_aarch64 --options "0 3" --cpu hpi --l1pf 2 --l1lat 1 --pflat 4
```
Output kept in `calib/r4_pflat4.txt`; full gem5 configuration in `calib/m5out_r4_pflat4/config.ini`. gem5 is deterministic: the same command gives the same numbers.

## Result against the board (no flush; board = mean of 40 repetitions on aup1, gem5 = mean of 3)

| routine | board A53 ns | gem5 ns | gem5 / board | share of processor time in a job |
|---|---|---|---|---|
| fill_secrets_280 | 13698 | 15480 | 1.13 | 8.8% |
| fill_chains_280 | 12962 | 13654 | 1.05 | 8.3% |
| fill_compress_8 | 2503 | 3157 | 1.26 | 1.6% |
| fill_level_4 | 366 | 421 | 1.15 | 0.7% |
| collect_280 | 1621 | 1785 | 1.10 | 2.1% |
| tags_copy_240kB | 156820 | 124272 | 0.79 | 36.6% |
| tags_collect_38kB | 8878 | 9395 | 1.06 | 2.1% |
| firmware_copy_1MB | 659018 | 646466 | 0.98 | 38.4% |
| poll_1000_reads | 2572 | 3402 | 1.32 | 1.5% |

Mean absolute error over the nine routines: **14.5%**. Worst: **32.3%** (poll_1000_reads). Weighted by processor time in a job: **11.2%**.
The weights are the calls of each routine in one job of 22 trees + 8 tag nodes + 2 firmware nodes times the board time per call; the two copies carry 75% of the time.

**The target (mean at or below 10%, worst at or below 20%) was NOT met.** The search was stopped at the agreed limit of 40 settings instead of adding more knobs.
The error has a clear shape: the two memory copies come out 2% and 21% faster than the board, the compute routines 5% to 32% slower.
No setting tried brings the tag copy and the 1 MB copy to the board together: with 8 L1D miss buffers they are 0.79 and 0.98; with 4 (the figure in ARM's own HPI cache set) 1.02 and 1.77.
The poll loop (4.1 cycles per iteration in gem5, 3.1 on the board) and the small-node layouts did not respond to any cache or prefetcher knob; they are set by the core model itself, which was not touched.

How the frozen setting was chosen: lowest time-weighted error, and it also has the lowest plain mean. Six more settings lie within 1.4 points of it on both measures (rows 2 to 7 below);
the round-3 best (`--cpu hpi --l1pf 2 --l1lat 1`, 15.1% / 31.8% / 12.6%) differs from the frozen one by at most 2.1% on any routine. The choice among them does not change any conclusion.

## Every setting tried (40, five rounds of 8), best plain mean first

Columns are gem5 / board per routine. All runs used `--options "0 3"`, 1.2 GHz unless stated. Nothing is left out; `calib/<name>.txt` holds each raw output.

| # | round | name | options | secrets | chains | compr8 | level4 | coll280 | tag copy | tag coll | fw copy | poll | mean | worst | time-weighted |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 4 | r4_pflat4 **(frozen)** | `--cpu hpi --l1pf 2 --l1lat 1 --pflat 4` | 1.13 | 1.05 | 1.26 | 1.15 | 1.10 | 0.79 | 1.06 | 0.98 | 1.32 | 14.5% | 32.3% | 11.2% |
| 2 | 5 | r5_m6 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 6` | 1.13 | 1.05 | 1.27 | 1.16 | 1.10 | 0.78 | 1.06 | 1.03 | 1.32 | 15.0% | 31.8% | 12.2% |
| 3 | 4 | r4_l1resp2 | `--cpu hpi --l1pf 2 --l1lat 1 --l1resp 2` | 1.13 | 1.06 | 1.27 | 1.17 | 1.10 | 0.78 | 1.06 | 0.97 | 1.32 | 15.1% | 31.8% | 12.2% |
| 4 | 3 | r3_hpi_pf2_l1lat1 | `--cpu hpi --l1pf 2 --l1lat 1` | 1.13 | 1.05 | 1.27 | 1.16 | 1.10 | 0.78 | 1.06 | 0.96 | 1.32 | 15.1% | 31.8% | 12.6% |
| 5 | 4 | r4_pfmiss | `--cpu hpi --l1pf 2 --l1lat 1 --pfmiss 1` | 1.13 | 1.05 | 1.27 | 1.16 | 1.10 | 0.78 | 1.06 | 0.96 | 1.32 | 15.1% | 31.8% | 12.6% |
| 6 | 4 | r4_l2lat14 | `--cpu hpi --l1pf 2 --l1lat 1 --l2lat 14` | 1.13 | 1.06 | 1.26 | 1.18 | 1.11 | 0.79 | 1.06 | 0.98 | 1.32 | 15.1% | 31.8% | 11.7% |
| 7 | 4 | r4_l2resp5 | `--cpu hpi --l1pf 2 --l1lat 1 --l2resp 5` | 1.13 | 1.05 | 1.32 | 1.16 | 1.10 | 0.79 | 1.06 | 0.98 | 1.32 | 15.2% | 32.2% | 11.2% |
| 8 | 5 | r5_m6_l2pf1 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 6 --l2pf 1` | 1.13 | 1.05 | 1.04 | 1.05 | 1.10 | 0.56 | 1.06 | 0.71 | 1.32 | 16.5% | 43.9% | 29.7% |
| 9 | 5 | r5_m4_l2pf2 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 4 --l2pf 2` | 1.13 | 1.05 | 0.90 | 1.05 | 1.10 | 0.52 | 1.06 | 0.68 | 1.36 | 18.3% | 47.6% | 32.2% |
| 10 | 5 | r5_m5_l2pf2 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 5 --l2pf 2` | 1.13 | 1.05 | 0.88 | 1.05 | 1.10 | 0.51 | 1.06 | 0.66 | 1.32 | 18.4% | 49.1% | 33.7% |
| 11 | 5 | r5_m6_l2pf2 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 6 --l2pf 2` | 1.13 | 1.05 | 0.92 | 1.20 | 1.10 | 0.51 | 1.06 | 0.66 | 1.32 | 19.7% | 49.3% | 33.7% |
| 12 | 5 | r5_m5_l2pf4 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 5 --l2pf 4` | 1.13 | 1.06 | 0.84 | 1.05 | 1.10 | 0.47 | 1.06 | 0.60 | 1.36 | 20.4% | 52.8% | 37.3% |
| 13 | 5 | r5_m4_l2pf4 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 4 --l2pf 4` | 1.13 | 1.06 | 0.78 | 1.05 | 1.10 | 0.47 | 1.06 | 0.60 | 1.32 | 20.7% | 52.9% | 37.4% |
| 14 | 3 | r3_minor_pf2_l1lat1 | `--cpu minor --l1pf 2 --l1lat 1` | 1.29 | 1.11 | 1.28 | 1.19 | 1.09 | 0.80 | 1.44 | 0.97 | 1.32 | 21.7% | 44.5% | 13.9% |
| 15 | 5 | r5_m4_l2pf8 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 4 --l2pf 8` | 1.13 | 1.07 | 0.81 | 1.05 | 1.10 | 0.43 | 1.06 | 0.54 | 1.32 | 21.7% | 57.1% | 41.6% |
| 16 | 3 | r3_minor_pf3_l1lat1 | `--cpu minor --l1pf 3 --l1lat 1` | 1.28 | 1.12 | 1.12 | 1.06 | 1.10 | 0.66 | 1.44 | 0.81 | 1.32 | 21.9% | 44.5% | 25.3% |
| 17 | 4 | r4_l1mshr4 | `--cpu hpi --l1pf 2 --l1lat 1 --l1mshr 4` | 1.13 | 1.06 | 1.33 | 1.20 | 1.10 | 1.02 | 1.06 | 1.77 | 1.32 | 22.1% | 77.4% | 33.6% |
| 18 | 4 | r4_hpicache | `--cpu hpi --l1pf 2 --l1lat 1 --l2lat 13 --l2resp 5 --l1mshr 4 --l2mshr 4 --wb 4 --pfq 4` | 1.13 | 1.06 | 1.42 | 1.17 | 1.10 | 1.08 | 1.06 | 1.86 | 1.32 | 24.5% | 86.1% | 39.3% |
| 19 | 4 | r4_l2lat14_r5_m4 | `--cpu hpi --l1pf 2 --l1lat 1 --l2lat 14 --l2resp 5 --l1mshr 4` | 1.13 | 1.06 | 1.47 | 1.17 | 1.11 | 1.10 | 1.06 | 1.88 | 1.32 | 25.7% | 88.3% | 41.0% |
| 20 | 1 | pf8_l2lat6_13 | `--l1pf 8 --l2pf 8 --l2lat 6 --clock 1.333GHz` | 1.17 | 1.06 | 0.83 | 0.99 | 1.25 | 0.50 | 1.30 | 0.57 | 1.77 | 29.6% | 77.2% | 39.4% |
| 21 | 3 | r3_hpi_pf2 | `--cpu hpi --l1pf 2` | 1.23 | 1.16 | 1.25 | 1.27 | 1.38 | 0.79 | 1.24 | 0.97 | 1.97 | 30.5% | 96.8% | 15.4% |
| 22 | 3 | r3_hpi_pf2_wb16 | `--cpu hpi --l1pf 2 --wb 16` | 1.23 | 1.16 | 1.25 | 1.27 | 1.38 | 0.79 | 1.24 | 0.97 | 1.97 | 30.5% | 96.8% | 15.4% |
| 23 | 3 | r3_hpi_pf2_l2lat9 | `--cpu hpi --l1pf 2 --l2lat 9` | 1.23 | 1.16 | 1.30 | 1.26 | 1.38 | 0.77 | 1.23 | 0.94 | 1.97 | 31.2% | 96.7% | 17.3% |
| 24 | 3 | r3_hpi_pf3 | `--cpu hpi --l1pf 3` | 1.23 | 1.17 | 1.17 | 1.13 | 1.38 | 0.63 | 1.24 | 0.80 | 1.97 | 31.8% | 96.9% | 27.7% |
| 25 | 3 | r3_hpi_pf2_l2pf1 | `--cpu hpi --l1pf 2 --l2pf 1` | 1.23 | 1.17 | 1.15 | 1.16 | 1.38 | 0.58 | 1.24 | 0.73 | 1.97 | 33.2% | 96.8% | 32.2% |
| 26 | 2 | r2_pf3_l2lat9 | `--l1pf 3 --l2lat 9` | 1.30 | 1.16 | 1.10 | 1.11 | 1.40 | 0.65 | 1.45 | 0.80 | 1.97 | 33.7% | 96.8% | 27.7% |
| 27 | 1 | pf_l1_4 | `--l1pf 4` | 1.30 | 1.16 | 1.04 | 1.14 | 1.39 | 0.62 | 1.46 | 0.78 | 1.97 | 34.0% | 96.9% | 29.7% |
| 28 | 2 | r2_pf2_l1only | `--l1pf 2` | 1.30 | 1.16 | 1.37 | 1.25 | 1.39 | 0.81 | 1.46 | 0.98 | 1.97 | 34.5% | 96.9% | 15.6% |
| 29 | 2 | r2_pf2 | `--l1pf 2 --l2pf 2` | 1.30 | 1.16 | 1.15 | 1.14 | 1.39 | 0.62 | 1.46 | 0.75 | 1.97 | 35.6% | 96.9% | 31.2% |
| 30 | 2 | r2_pf2_16x4 | `--l1pf 2 --l2pf 2 --mem DDR4_2400_16x4` | 1.30 | 1.16 | 1.15 | 1.14 | 1.39 | 0.62 | 1.46 | 0.75 | 1.97 | 35.6% | 96.9% | 31.2% |
| 31 | 1 | hpi_pf8 | `--cpu hpi --l1pf 8 --l2pf 8 --l2lat 6` | 1.23 | 1.17 | 0.84 | 1.11 | 1.37 | 0.48 | 1.23 | 0.54 | 1.97 | 35.7% | 96.7% | 43.0% |
| 32 | 2 | r2_pf4_16x4 | `--l1pf 4 --l2pf 4 --mem DDR4_2400_16x4` | 1.30 | 1.17 | 0.97 | 1.11 | 1.38 | 0.56 | 1.46 | 0.63 | 1.97 | 35.8% | 96.9% | 37.4% |
| 33 | 1 | pf_l1_8_l2_8 | `--l1pf 8 --l2pf 8` | 1.30 | 1.17 | 0.94 | 1.11 | 1.39 | 0.56 | 1.46 | 0.63 | 1.97 | 36.2% | 96.9% | 37.6% |
| 34 | 2 | r2_pf2_l2pf4 | `--l1pf 2 --l2pf 4` | 1.31 | 1.17 | 1.09 | 1.15 | 1.39 | 0.58 | 1.46 | 0.69 | 1.97 | 36.2% | 96.9% | 35.0% |
| 35 | 1 | pf8_l2lat6 | `--l1pf 8 --l2pf 8 --l2lat 6` | 1.30 | 1.17 | 0.93 | 1.09 | 1.39 | 0.55 | 1.45 | 0.61 | 1.97 | 36.4% | 96.8% | 38.8% |
| 36 | 1 | pf8_simple20 | `--l1pf 8 --l2pf 8 --l2lat 6 --mem simple --memlat 20ns` | 1.29 | 1.16 | 0.89 | 1.06 | 1.37 | 0.52 | 1.45 | 0.56 | 1.97 | 36.8% | 96.6% | 41.8% |
| 37 | 2 | r2_pf4_simple60 | `--l1pf 4 --l2pf 4 --mem simple --memlat 60ns` | 1.30 | 1.17 | 0.94 | 1.15 | 1.40 | 0.54 | 1.46 | 0.59 | 1.97 | 37.5% | 97.2% | 39.9% |
| 38 | 2 | r2_pf1 | `--l1pf 1 --l2pf 1` | 1.31 | 1.16 | 1.64 | 1.28 | 1.38 | 0.71 | 1.45 | 0.87 | 1.97 | 40.3% | 96.9% | 24.1% |
| 39 | 1 | o3_pf8 | `--cpu o3 --l1pf 8 --l2pf 8 --l2lat 6` | 0.38 | 0.38 | 0.30 | 0.39 | 0.64 | 0.26 | 0.35 | 0.45 | 0.35 | 61.1% | 74.5% | 63.6% |
| 40 | 1 | base | `(stock: in-order Minor core, no prefetcher)` | 1.30 | 1.53 | 2.81 | 1.81 | 1.59 | 2.29 | 2.82 | 3.74 | 1.97 | 120.6% | 273.6% | 169.2% |

Notes on the list: `r4_pfmiss` (prefetcher trained on misses only) and `r3_hpi_pf2_wb16` (16 write buffers) reproduce their parent setting to the digit, so those two knobs do nothing here.
The out-of-order core (`o3_pf8`) is 2.6 times too fast and was dropped after round 1. Every setting with an L2 prefetcher makes the copies too fast (13% to 57%).

## What this model cannot give

1. **The cache-flush instruction.** In SE mode gem5 does not execute the data-cache flush (the two `*_flush_only` rows come out at about 140 ns, i.e. nothing). The binary is therefore always run with flush = 0 in gem5,
   and the flush cost stays a board measurement: 28.5 us per 240 kB and 124.6 us per 1 MB (13% to 15% of the copy), plus what the flush adds to the routine that next reads the flushed lines
   (tag collect: 8.9 us without flush, 31.3 us with flush on the board).
2. **Register access to the real device.** `poll_1000_reads` and `shape_5_writes` touch ordinary memory in this binary, on the board and in gem5. The latency of a read or write that crosses the bus to an atom's registers is not in the model.
3. **The rest of the system.** One core, no operating system, no interrupts, no other core sharing the L2; the board numbers are means under Linux and include that noise (board mean is 1% to 64% above board minimum, by routine).
4. **Repetitions.** gem5 runs 3 repetitions (the first one cold) against 40 on the board; for the tag copy the gem5 minimum is 8% below its mean.
5. **The atoms and the memory port.** Not modelled; measured.

Files: `ruty_se2.py` (model; round-4 options added with defaults that reproduce rounds 1 to 3), `calib_grid.sh` to `calib_grid5.sh` (the five rounds), `calib_table.py` (text table), `make_frozen.py` (writes this file), `calib/` (raw outputs and gem5 output directories).
