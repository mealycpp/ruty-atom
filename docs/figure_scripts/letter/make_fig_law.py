#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""fig_law: gain of a 1.80x faster core against f, the share of time outside the processor's per-call work.
Points: only where f is measured independently (PAPER_FACTS FINAL, SIGN_RESULT.md; Karl et al. Table 4 with SLotH's call count)."""
import numpy as np, matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import rcParams
rcParams["font.family"] = "STIXGeneral"; rcParams["mathtext.fontset"] = "stix"; rcParams["pdf.fonttype"] = 42; rcParams["axes.linewidth"] = 0.6
import re, os
_keys = re.findall(r"\\bibitem\{([^}]*)\}", open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "main.bbl")).read())
def ref(k): return "[%d]" % (_keys.index(k) + 1)
B = "#1f4e9c"; FS = 6.4; g = 2.0; RS = 24 / 26.87   # rounds halve; 24 of 26.87 clocks of a chain step are rounds
fig, ax = plt.subplots(figsize=(3.45, 1.32)); fig.subplots_adjust(left=0.115, right=0.985, top=0.97, bottom=0.2)
f = np.linspace(0, 1, 200); ax.plot(f, 1 / ((1 - f) + f / g), "-", color="k", lw=0.9, zorder=2)
ax.text(0.2, 1.62, "Amdahl's law " + ref("amdahl1967"), fontsize=6.6, ha="center")
ax.text(0.2, 1.42, r"$G=\dfrac{1}{(1-f)+f/2}$", fontsize=7.2, ha="center")
P = [(2.182 / 9.609, 9.779 / 6.807, "1 atom"), (2.227 / 5.553, 5.556 / 3.966, "2"), (2.266 / 3.617, 3.683 / 3.002, "4")]
for p, G, t in P:
    ff = (1 - p) * RS
    ax.plot([ff, ff], [G, 1 / ((1 - ff) + ff / g)], "-", color=B, lw=1.1, zorder=3)
    ax.plot([ff], [G], "o", ms=4.6, mfc=B, mec=B, zorder=4)
    ax.annotate(t, (ff, G), textcoords="offset points", xytext=(0, -9.5), ha="center", fontsize=6.0, color=B)
ax.annotate("full signature,\nRUTY-ATOM", (0.535, 1.401), textcoords="offset points", xytext=(-2, 14), ha="center", fontsize=6.0, color=B)
ax.plot([RS], [1.80], "s", ms=4.4, mfc=B, mec=B, zorder=4); ax.annotate("chain step\nin the atom", (RS, 1.80), textcoords="offset points", xytext=(-7, 2), ha="right", va="center", fontsize=6.0, color=B)
fk = 24 / 406.0
ax.plot([fk], [1.024], "s", ms=4.6, mfc="w", mec="k", mew=0.9, zorder=4); ax.annotate("Karl et al. " + ref("karl2025"), (fk, 1.024), textcoords="offset points", xytext=(7, -1), ha="left", va="center", fontsize=6.2)
ax.set_xlim(0, 1); ax.set_ylim(0.97, 2.03); ax.set_xticks([0, 0.2, 0.4, 0.6, 0.8, 1.0]); ax.set_yticks([1.0, 1.2, 1.4, 1.6, 1.8, 2.0])
ax.set_xlabel(r"$f$: share of time spent in permutation rounds", fontsize=FS, labelpad=1); ax.set_ylabel(r"Gain from 12 rounds, $G$", fontsize=FS, labelpad=2)
ax.tick_params(labelsize=6.0, width=0.5, length=2.2, pad=1.5); ax.grid(color="0.9", lw=0.4); ax.set_axisbelow(True)
for s in ("top", "right"): ax.spines[s].set_visible(False)
fig.savefig("fig_law.pdf", bbox_inches="tight", pad_inches=0.01); fig.savefig("fig_law.png", dpi=300, bbox_inches="tight", pad_inches=0.01)
for p, G, t in P: print(t, "f=%.3f pred=%.3f meas=%.3f" % ((1 - p) * RS, 1 / ((1 - (1 - p) * RS) + (1 - p) * RS / g), G))
print("chain f=%.3f pred=%.3f" % (RS, 1 / ((1 - RS) + RS / g)))
print("karl f=%.3f pred=%.4f" % (fk, 1 / ((1 - fk) + fk / g)))
