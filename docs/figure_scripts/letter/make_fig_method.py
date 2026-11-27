#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""fig_method (single column): gem5 + board times -> simulator -> two decisions: how many atoms (hardware), which scheduler (software).
Numbers: sim/design_ranking.txt (model, written before the no-copy build) and PAPER_FACTS (board)."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle, FancyArrowPatch
from matplotlib import rcParams
rcParams["font.family"] = "STIXGeneral"; rcParams["mathtext.fontset"] = "stix"; rcParams["pdf.fonttype"] = 42
rcParams["hatch.linewidth"] = 0.4; rcParams["axes.linewidth"] = 0.6
W, H = 3.45, 1.72; FS = 6.4; B = "#1f4e9c"
fig = plt.figure(figsize=(W, H))
fx = fig.add_axes([0, 0.80, 1, 0.20]); fx.set_xlim(0, W); fx.set_ylim(0, 0.384); fx.axis("off")
def bx(x, w, t, fc="white", lw=0.7):
    fx.add_patch(Rectangle((x, 0.07), w, 0.24, fc=fc, ec="k", lw=lw, zorder=3)); fx.text(x + w / 2, 0.19, t, ha="center", va="center", fontsize=FS, zorder=4)
def ar(x0, x1): fx.add_patch(FancyArrowPatch((x0, 0.19), (x1, 0.19), arrowstyle="-|>", mutation_scale=5, lw=0.6, color="k", shrinkA=0, shrinkB=0))
bx(0.03, 0.80, "gem5: Cortex-A53"); bx(0.83, 0.82, "board: atom, port"); bx(1.85, 0.62, "simulator", fc="0.92", lw=1.0); bx(2.67, 0.75, "forecast filed", lw=0.7)
ar(1.65, 1.85); ar(2.47, 2.67)
def panel(l, w, title):
    ax = fig.add_axes([l / W, 0.19, w / W, 0.47]); ax.set_title(title, fontsize=FS, pad=2.5, fontweight="bold")
    ax.tick_params(labelsize=5.8, width=0.5, length=2.2, pad=1.5)
    for s in ("top", "right"): ax.spines[s].set_visible(False)
    return ax
ax = panel(0.34, 1.20, "Hardware: how many atoms?")
n = [1, 2, 4, 6, 8, 12, 16]
tr = [7.741, 4.036, 2.360, 1.703, 1.419, 1.123, 1.122]; mx = [10.245, 5.397, 2.780, 1.910, 1.913, 1.925, 1.951]
ax.axvspan(4.6, 19, color="0.93", lw=0)
ax.plot(n, tr, "-", color="k", lw=0.9, marker="o", ms=2.4, mfc="k", label="trees")
ax.plot(n, mx, "--", color="k", lw=0.9, marker="s", ms=2.4, mfc="w", mew=0.6, label="mixed job")
ax.plot([1, 2, 4, 2, 4], [7.48, 4.15, 2.33, 5.720, 3.354], ls="none", marker="x", ms=4.5, color=B, mew=1.0, label="board")
ax.set_xscale("log", base=2); ax.set_xticks([1, 2, 4, 8, 16]); ax.set_xticklabels(["1", "2", "4", "8", "16"]); ax.set_xlim(0.85, 19)
ax.set_ylim(0, 11); ax.set_yticks([0, 5, 10]); ax.set_ylabel("Job time (ms)", fontsize=6.0, labelpad=1); ax.set_xlabel("atoms (model)", fontsize=6.0, labelpad=0.5)
ax.legend(fontsize=5.4, frameon=False, loc="upper right", handlelength=1.5, labelspacing=0.15, borderaxespad=0.0)
ax = panel(2.30, 1.10, "Software: which scheduler?")
opt = ["no copy", "second core", "merged node", "split chains", "split tags"]; gain = [43.9, 43.9, 6.1, 1.8, -1.0]
y = list(range(len(opt)))[::-1]
ax.barh(y, gain, 0.6, fc="white", ec="k", lw=0.7, hatch="////")
ax.plot([42.1], [4], marker="x", ms=5.5, color=B, mew=1.2, ls="none", zorder=5); ax.text(47, 4, "board", color=B, fontsize=5.6, ha="left", va="center")
ax.set_yticks(y); ax.set_yticklabels(opt, fontsize=5.8); ax.set_xlim(-4, 66); ax.set_xticks([0, 20, 40]); ax.set_xlabel("time saved, model (%)", fontsize=6.0, labelpad=0.5)
ax.axvline(0, color="k", lw=0.5)
fig.savefig("fig_method.pdf", bbox_inches="tight", pad_inches=0.01); fig.savefig("fig_method.png", dpi=300, bbox_inches="tight", pad_inches=0.01); print("ok")
