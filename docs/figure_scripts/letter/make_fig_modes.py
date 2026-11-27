#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""fig_modes: one application per hash mode, RUTY-ATOM against prior work, with the law/forecast curve.
MEAS entries stay None until the board numbers exist (results\\modes_* on the PC); nothing unmeasured is drawn as measured."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib import rcParams
rcParams["font.family"] = "STIXGeneral"; rcParams["mathtext.fontset"] = "stix"; rcParams["pdf.fonttype"] = 42; rcParams["axes.linewidth"] = 0.6
import re, os
_bbl = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "main.bbl")
_keys = re.findall(r"\\bibitem\{([^}]*)\}", open(_bbl).read())
def ref(k): return " [%d]" % (_keys.index(k) + 1)   # reference numbers follow the compiled letter
B = "#1f4e9c"; K = "0.12"; FS = 6.4; N = [1, 2, 4]; F = {1: 450, 2: 420, 4: 400}
MEAS = dict(
    kt_gbps=None,            # KT128, 16 MB, Gb/s at N = 1, 2, 4
    kt_a53=None,             # (1 core, 4 cores) Gb/s, C on the board's A53
    tree_split=None, tree_turbo=None, tree_shake=None,   # M hash/s at N = 1, 2, 4, k = 16
    tree_xkcp=None,          # (MiniSHAKE128 1 core, 4 cores) M hash/s, XKCP permutation on the A53
)
fig, axs = plt.subplots(1, 3, figsize=(7.1, 1.34))
fig.subplots_adjust(left=0.055, right=0.995, top=0.88, bottom=0.2, wspace=0.27)
def style(ax, title, ylabel):
    ax.set_title(title, fontsize=FS + 0.4, pad=2.5); ax.set_xlabel("atoms $N$", fontsize=FS, labelpad=0.5); ax.set_ylabel(ylabel, fontsize=FS, labelpad=1)
    ax.set_xticks(N); ax.set_xlim(0.7, 4.3); ax.tick_params(labelsize=6.0, width=0.5, length=2.2, pad=1.5); ax.grid(color="0.9", lw=0.4); ax.set_axisbelow(True)
STY = [((0, (5, 2)), "s"), ((0, (1, 1.2)), "^"), ((0, (5, 1.5, 1, 1.5)), "D"), ((0, (8, 2, 1, 2, 1, 2)), "v")]   # one dash pattern and marker per other design
_cnt = {}
def hline(ax, y, name, x=4.25, va="bottom"):
    k = _cnt.get(id(ax), 0); _cnt[id(ax)] = k + 1; ls, mk = STY[k % len(STY)]
    ax.axhline(y, color=K, lw=0.75, ls=ls); ax.plot([0.78], [y], marker=mk, ms=3.8, mfc="w", mec=K, mew=0.8, ls="none", clip_on=False)
    ax.annotate(name, (x, y), ha="right", va=va, fontsize=5.8, color=K, xytext=(0, 1 if va == "bottom" else -1), textcoords="offset points")
# (a) standard signature, SHAKE256
ax = axs[0]; style(ax, "(a) SHAKE256: SLH-DSA-128f signature", "ms")
tp, ta = 1.157, 8.556; xs = np.linspace(1, 4, 61); fx = np.interp(xs, [1, 2, 4], [450, 420, 400])
ax.plot(xs, tp + ta / xs * 450 / fx, "-", color=B, lw=0.8, label="law $t_p+(t_a/N)(450/F_N)$")
ax.plot(N, [9.78, 5.56, 3.68], "o", ms=4.2, color=B, ls="none", label="RUTY-ATOM, measured")
for y, name, x, va in ((49.0, "SLotH" + ref("saarinen2024"), 4.25, "bottom"), (19.3, "SPHINCSLET" + ref("deshpande2025"), 4.25, "bottom"), (13.19, "Trident$^*$" + ref("bao2026"), 4.25, "top"), (0.263, "Sun et al." + ref("sun2026"), 4.25, "bottom")): hline(ax, y, name, x=x, va=va)
ax.set_yscale("log"); ax.set_ylim(0.15, 120); ax.set_yticks([0.3, 1, 3, 10, 30, 100]); ax.set_yticklabels(["0.3", "1", "3", "10", "30", "100"]); ax.minorticks_off()
ax.legend(fontsize=5.4, frameon=False, loc="lower left", bbox_to_anchor=(0.0, 0.08), handlelength=1.6, labelspacing=0.2, borderaxespad=0.2)
# (b) KangarooTwelve, TurboSHAKE128  (measured: results\\modes_kt_*; forecast filed 19:23 before the run)
ax = axs[1]; style(ax, "(b) TurboSHAKE128: KangarooTwelve, 16 MB", "Gb/s")
ax.plot(N, [17.4, 19.0, 19.0], ls="none", marker="D", ms=5.6, mfc="none", mec="0.25", mew=0.7, label="forecast, filed before the run", zorder=2)
ax.plot(N, [17.48, 19.08, 19.07], "-", color=B, lw=0.9, marker="o", ms=4.2, label="RUTY-ATOM, measured", zorder=3)
hline(ax, 3.5 / 0.51 * 8, "SkylakeX, AVX-512, software" + ref("keccakperf"))          # 0.51 c/B at 3.5 GHz, keccak.team
hline(ax, 3.2 / 1.41 * 8, "Haswell, AVX2, software" + ref("keccakperf"), x=3.55, va="top")   # 1.41 c/B at 3.2 GHz, keccak.team
hline(ax, 0.678 * 8, "this board, 4 $\\times$ Cortex-A53"); hline(ax, 0.174 * 8, "this board, 1 $\\times$ Cortex-A53")
ax.set_yscale("log"); ax.set_ylim(0.9, 400); ax.set_yticks([1, 3, 10, 30, 100]); ax.set_yticklabels(["1", "3", "10", "30", "100"]); ax.minorticks_off()
ax.legend(fontsize=5.4, frameon=False, loc="upper left", handlelength=1.6, labelspacing=0.2, borderaxespad=0.3)
# (c) Merkle tree, 2^16 leaves (measured: results\\modes_merkle_*, modes_xkcp_sw_a53_aup1.txt)
ax = axs[2]; style(ax, "(c) MiniSHAKE128: Merkle tree, $2^{16}$ leaves", "M hash/s")
H = 131071 / 1e6
ax.plot(N, [H / (x / 1e3) for x in (6.5, 4.1, 4.2)], ls="none", marker="D", ms=5.6, mfc="none", mec="0.25", mew=0.7, label="forecast (split)", zorder=2)
ax.plot(N, [20.9, 33.1, 33.5], "-", color=B, lw=0.9, marker="o", ms=4.2, label="RUTY-ATOM, MiniSHAKE128, split", zorder=4)
ax.plot(N, [15.5, 28.6, 33.1], "--", color=B, lw=0.8, marker="o", ms=3.8, mfc="w", mec=B, mew=0.9, label="RUTY-ATOM, TurboSHAKE128, wide", zorder=3)
ax.plot(N, [11.0, 20.4, 31.8], ":", color=B, lw=0.9, marker="s", ms=3.4, mfc="w", mec=B, mew=0.9, label="RUTY-ATOM, SHAKE128, wide", zorder=3)
hline(ax, H / 33.5e-3, "designers' C code" + ref("xkcp") + ", 4 $\\times$ Cortex-A53"); hline(ax, H / 133.7e-3, "designers' C code" + ref("xkcp") + ", 1 $\\times$ Cortex-A53")
ax.set_yscale("log"); ax.set_ylim(0.025, 60); ax.set_yticks([0.1, 1, 10]); ax.set_yticklabels(["0.1", "1", "10"]); ax.minorticks_off()
ax.legend(fontsize=5.2, frameon=False, loc="lower right", handlelength=2.0, labelspacing=0.2, borderaxespad=0.2)
fig.savefig("fig_modes.pdf", bbox_inches="tight", pad_inches=0.01); fig.savefig("fig_modes.png", dpi=220, bbox_inches="tight", pad_inches=0.01); print("ok")
