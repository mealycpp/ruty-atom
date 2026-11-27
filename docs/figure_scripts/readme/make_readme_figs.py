#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""README figures for RUTY-ATOM, on the banner's dark surface.
Every number is a measured value from PAPER_FACTS.md / MODES_RESULT.md or a published value cited in the letter.
Palette validated with the dataviz validator on surface #0b1a12 (dark): gold #c2932f, blue #338ced, aqua #00a888.
Other work is drawn in neutral grey (context, not a series)."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager as fm, rcParams
from matplotlib.patches import FancyBboxPatch, Rectangle, FancyArrowPatch
import numpy as np

G = "/usr/share/fonts/truetype/google-fonts/ofl/"
for f in ("firasans/FiraSans-Regular.ttf", "firasans/FiraSans-Medium.ttf", "firasans/FiraSans-Light.ttf", "firasans/FiraSans-SemiBold.ttf", "cardo/Cardo-Bold.ttf", "cardo/Cardo-Regular.ttf"):
    fm.fontManager.addfont(G + f)
rcParams["font.family"] = "Fira Sans"; rcParams["mathtext.fontset"] = "dejavusans"
BG = "#0b1a12"; CARD = "#10241a"; EDGE = "#1d3a2b"; TXT = "#ece2c8"; SUB = "#9aa69f"; GRID = "#1a3024"
GOLD = "#c2932f"; BLUE = "#338ced"; AQUA = "#00a888"; GREY = "#7d8a83"
OUT = "docs/img/"

def save(fig, name):
    fig.savefig(OUT + name, dpi=200, facecolor=BG, bbox_inches="tight", pad_inches=0.18); plt.close(fig); print(name)

def axstyle(ax, title, sub=None):
    ax.set_facecolor(CARD)
    for s in ax.spines.values(): s.set_color(EDGE); s.set_linewidth(0.8)
    ax.tick_params(colors=SUB, labelsize=9, length=0, pad=5)
    ax.grid(color=GRID, lw=0.8); ax.set_axisbelow(True)
    ax.set_title(title, color=TXT, fontsize=12.5, fontweight="medium", loc="left", pad=22 if sub else 10)
    if sub: ax.text(0, 1.035, sub, transform=ax.transAxes, color=SUB, fontsize=9, va="bottom")

# ---------------------------------------------------------------- 1. stat tiles
def stats():
    tiles = [("645 LUTs", "cost of the second width", "8.7% over a fixed-width build"),
             ("480 MHz", "no clock cost", "fixed and splittable builds both meet it"),
             ("3.68 ms", "SLH-DSA-SHAKE-128f signature", "four atoms, byte-identical to FIPS 205"),
             ("1.90 ms", "firmware authentication", "1 MiB image hashed and verified, one atom"),
             ("19.1 Gb/s", "KangarooTwelve", "two atoms fill the memory port"),
             ("33 M/s", "Merkle-tree hashes", "four atoms, 2¹⁶ leaves")]
    fig = plt.figure(figsize=(12, 3.5)); fig.patch.set_facecolor(BG)
    for k, (big, mid, small) in enumerate(tiles):
        r, c = divmod(k, 3); ax = fig.add_axes([0.01 + c * 0.332, 0.52 - r * 0.50, 0.318, 0.45]); ax.axis("off"); ax.set_xlim(0, 1); ax.set_ylim(0, 1)
        ax.add_patch(FancyBboxPatch((0.005, 0.03), 0.99, 0.94, boxstyle="round,pad=0,rounding_size=0.045", fc=CARD, ec=EDGE, lw=1.0, mutation_aspect=2.3))
        ax.add_patch(Rectangle((0.045, 0.2), 0.012, 0.6, fc=GOLD, ec="none"))
        ax.text(0.09, 0.62, big, color=TXT, fontsize=27, fontweight="semibold", va="center")
        ax.text(0.09, 0.33, mid, color=GOLD, fontsize=11.5, fontweight="medium", va="center")
        ax.text(0.09, 0.16, small, color=SUB, fontsize=9.5, va="center")
    save(fig, "stats.png")

# ---------------------------------------------------------------- 2. the split
def split():
    fig = plt.figure(figsize=(12, 3.9)); fig.patch.set_facecolor(BG); ax = fig.add_axes([0, 0, 1, 1]); ax.axis("off"); ax.set_xlim(0, 12); ax.set_ylim(0, 3.9)
    def grid(x0, y0, mode):
        c = 0.42; g = 0.07
        for r in range(5):
            for q in range(5):
                x, y = x0 + q * (c + g), y0 + r * (c + g)
                if mode == 0: ax.add_patch(Rectangle((x, y), c, c, fc=GOLD, ec="none"))
                else:
                    ax.add_patch(Rectangle((x, y), c / 2 - 0.015, c, fc=GOLD, ec="none")); ax.add_patch(Rectangle((x + c / 2 + 0.015, y), c / 2 - 0.015, c, fc=BLUE, ec="none"))
    ax.text(0.45, 3.5, "One register, two shapes", color=TXT, fontsize=17, fontweight="medium")
    ax.text(0.45, 3.15, "25 lanes of 64 bits. The bit dual decides what they are.", color=SUB, fontsize=11)
    grid(0.55, 0.42, 0); grid(7.6, 0.42, 1)
    ax.text(3.25, 2.35, "dual = 0", color=GOLD, fontsize=13, fontweight="medium"); ax.text(3.25, 1.98, "one Keccak-p[1600] state", color=TXT, fontsize=12)
    ax.text(3.25, 1.64, "SHAKE128 / 256  ·  24 rounds", color=SUB, fontsize=10.5); ax.text(3.25, 1.36, "TurboSHAKE128 / 256  ·  12 rounds", color=SUB, fontsize=10.5)
    ax.text(10.3, 2.35, "dual = 1", color=BLUE, fontsize=13, fontweight="medium"); ax.text(10.3, 1.98, "two Keccak-p[800]", color=TXT, fontsize=12); ax.text(10.3, 1.7, "states, side by side", color=TXT, fontsize=12)
    ax.text(10.3, 1.36, "MiniSHAKE128 / 256", color=SUB, fontsize=10.5); ax.text(10.3, 1.08, "12 rounds, two jobs at once", color=SUB, fontsize=10.5)
    ax.add_patch(FancyArrowPatch((6.15, 1.72), (7.35, 1.72), arrowstyle="<|-|>", mutation_scale=18, color=TXT, lw=1.6))
    ax.text(6.75, 2.02, "one register write", color=TXT, fontsize=10.5, ha="center"); ax.text(6.75, 1.36, "no FPGA", color=SUB, fontsize=9.5, ha="center"); ax.text(6.75, 1.12, "reconfiguration", color=SUB, fontsize=9.5, ha="center")
    ax.text(0.55, 0.12, "one round per clock in both shapes", color=SUB, fontsize=10)
    save(fig, "split.png")

# ---------------------------------------------------------------- 3. where it stands (three applications)
def compare():
    fig, axs = plt.subplots(1, 3, figsize=(13.6, 4.9)); fig.patch.set_facecolor(BG); fig.subplots_adjust(left=0.125, right=0.985, top=0.79, bottom=0.17, wspace=0.62)
    # (a) signature: dot plot on a log axis (no bars: a log axis has no zero)
    ax = axs[0]; axstyle(ax, "SLH-DSA-SHAKE-128f signature", "time to sign, ms (log scale, lower is better)")
    rows = [("Sun et al.  (64,084 LUTs)", 0.263, GREY), ("RUTY-ATOM, 4 atoms", 3.68, GOLD), ("RUTY-ATOM, 2 atoms", 5.56, GOLD), ("RUTY-ATOM, 1 atom", 9.78, GOLD),
            ("Trident, hypertree only", 13.19, GREY), ("SPHINCSLET", 19.30, GREY), ("SLotH", 49.0, GREY), ("Karl et al.", 284.0, GREY)]
    for i, (n, v, col) in enumerate(rows[::-1]):
        ax.plot([0.1, v], [i, i], color=col, lw=2.2, alpha=0.55, solid_capstyle="round"); ax.plot([v], [i], "o", ms=9, color=col, mec=CARD, mew=1.6)
        ax.text(v * 1.28, i, ("%.2f" % v if v < 1 else "%.4g" % v), color=TXT, fontsize=9.5, va="center")
    ax.set_yticks(range(len(rows))); ax.set_yticklabels([r[0] for r in rows[::-1]], fontsize=9.3)
    for t, r in zip(ax.get_yticklabels(), rows[::-1]): t.set_color(TXT if r[2] == GOLD else SUB)
    ax.set_xscale("log"); ax.set_xlim(0.1, 1300); ax.set_xticks([0.1, 1, 10, 100, 1000]); ax.set_xticklabels(["0.1", "1", "10", "100", "1000"]); ax.grid(axis="y", lw=0); ax.set_ylim(-0.6, len(rows) - 0.4)
    # (b) KangarooTwelve: bars, linear, zero baseline
    ax = axs[1]; axstyle(ax, "KangarooTwelve, 16 MiB", "throughput, Gb/s (higher is better)")
    rows = [("A53, 1 core *", 1.39, GREY), ("A53, 4 cores *", 5.42, GREY), ("RUTY-ATOM, 1 atom", 17.48, GOLD), ("RUTY-ATOM, 2 atoms", 19.08, GOLD), ("RUTY-ATOM, 4 atoms", 19.07, GOLD),
            ("Haswell, AVX2 †", 3.2 / 1.41 * 8, GREY), ("SkylakeX, AVX-512 †", 3.5 / 0.51 * 8, GREY)]
    for i, (n, v, col) in enumerate(rows[::-1]):
        ax.barh(i, v, height=0.56, color=col, ec="none"); ax.text(max(v, 19.2 if v > 15 else 0) + 1.3, i, "%.1f" % v, color=TXT, fontsize=9.5, va="center")
    ax.axvline(19.2, color=TXT, lw=1.0, ls=(0, (3, 3))); ax.text(20.5, len(rows) - 0.72, "memory port, 19.2", color=TXT, fontsize=8.8, va="center")
    ax.set_yticks(range(len(rows))); ax.set_yticklabels([r[0] for r in rows[::-1]], fontsize=9.3)
    for t, r in zip(ax.get_yticklabels(), rows[::-1]): t.set_color(TXT if r[2] == GOLD else SUB)
    ax.set_xlim(0, 66); ax.grid(axis="y", lw=0); ax.set_ylim(-0.6, len(rows) - 0.25)
    # (c) Merkle tree: lines over atoms, linear
    ax = axs[2]; axstyle(ax, "Merkle tree, 2¹⁶ leaves", "million hashes per second (higher is better)")
    N = [1, 2, 4]
    for lab, y, col, mk in (("MiniSHAKE128, split state", [20.9, 33.1, 33.5], GOLD, "o"), ("TurboSHAKE128, wide state", [15.5, 28.6, 33.1], BLUE, "s"), ("SHAKE128, wide state", [11.0, 20.4, 31.8], AQUA, "D")):
        ax.plot(N, y, "-", color=col, lw=2.2, marker=mk, ms=8, mec=CARD, mew=1.6, label=lab, zorder=3)
    ax.text(1.0, 22.6, "20.9", color=TXT, fontsize=9.5, ha="center"); ax.text(4.0, 35.4, "33.5", color=TXT, fontsize=9.5, ha="center")
    ax.axhline(3.91, color=GREY, lw=1.6, ls=(0, (5, 3))); ax.text(4.12, 4.9, "Keccak team's C code, 4 × Cortex-A53: 3.9", color=SUB, fontsize=8.8, ha="right")
    ax.axhline(0.98, color=GREY, lw=1.6, ls=(0, (1, 2))); ax.text(4.12, 1.9, "1 core: 1.0", color=SUB, fontsize=8.8, ha="right")
    ax.set_xticks(N); ax.set_xlim(0.75, 4.25); ax.set_ylim(0, 40); ax.set_xlabel("atoms", color=SUB, fontsize=9.5, labelpad=4)
    leg = ax.legend(loc="lower right", bbox_to_anchor=(1.0, 0.16), fontsize=8.8, frameon=False, labelcolor=TXT, handlelength=2.2, labelspacing=0.35)
    fig.text(0.125, 0.03, "* measured on the same board, plain C      † converted from the designers' published cycles per byte at the stated clock      Other designs: values as published, on their own devices.", color=SUB, fontsize=8.6)
    fig.text(0.125, 0.93, "Where it stands: one application per hash family", color=TXT, fontsize=16, fontweight="medium")
    save(fig, "compare.png")

# ---------------------------------------------------------------- 4. Amdahl
def amdahl():
    fig, ax = plt.subplots(figsize=(8.4, 4.3)); fig.patch.set_facecolor(BG); fig.subplots_adjust(left=0.09, right=0.97, top=0.80, bottom=0.14)
    axstyle(ax, "", None)
    f = np.linspace(0, 1, 200); ax.plot(f, 1 / ((1 - f) + f / 2), "-", color=TXT, lw=2.0, zorder=2, label="Amdahl's law, speed-up 2 on the share f")
    RS = 24 / 26.87
    P = [(2.182 / 9.609, 9.779 / 6.807, "1 atom"), (2.227 / 5.553, 5.556 / 3.966, "2 atoms"), (2.266 / 3.617, 3.683 / 3.002, "4 atoms")]
    for k, (p, Gm, t) in enumerate(P):
        ff = (1 - p) * RS; ax.plot([ff, ff], [Gm, 1 / ((1 - ff) + ff / 2)], color=GOLD, lw=2.0, zorder=3)
        ax.plot([ff], [Gm], "o", ms=10, color=GOLD, mec=CARD, mew=1.6, zorder=4, label="full signature, measured" if k == 0 else None)
        ax.annotate(t, (ff, Gm), textcoords="offset points", xytext=(0, 11) if k == 1 else (0, -17), ha="center", color=TXT, fontsize=9.5)
    ax.plot([RS], [1.80], "s", ms=10, color=BLUE, mec=CARD, mew=1.6, zorder=4, label="hash-chain step in the atom, measured"); ax.annotate("1.80×", (RS, 1.80), textcoords="offset points", xytext=(-12, 6), ha="right", color=TXT, fontsize=9.5)
    ax.plot([24 / 406.0], [1.024], "D", ms=9, color=GREY, mec=CARD, mew=1.6, zorder=4, label="Karl et al., from their cycle counts"); ax.annotate("2.4%", (24 / 406.0, 1.024), textcoords="offset points", xytext=(10, 8), color=SUB, fontsize=9.5)
    ax.set_xlim(0, 1); ax.set_ylim(0.96, 2.04); ax.set_xlabel("f : share of time spent in permutation rounds", color=SUB, fontsize=10, labelpad=6); ax.set_ylabel("gain from halving the rounds", color=SUB, fontsize=10, labelpad=6)
    ax.legend(loc="upper left", fontsize=9.3, frameon=False, labelcolor=TXT, labelspacing=0.5, handlelength=2.2)
    fig.text(0.09, 0.925, "One law behind the gains", color=TXT, fontsize=16, fontweight="medium"); fig.text(0.09, 0.865, "A faster core pays in proportion to the time spent in it. Bars show the distance between the board and the law.", color=SUB, fontsize=9.6)
    save(fig, "amdahl.png")

# ---------------------------------------------------------------- 5. system at a glance
def system():
    fig = plt.figure(figsize=(12, 4.6)); fig.patch.set_facecolor(BG); ax = fig.add_axes([0, 0, 1, 1]); ax.axis("off"); ax.set_xlim(0, 12); ax.set_ylim(0, 4.6)
    def box(x, y, w, h, title, lines=(), ec=EDGE, tc=TXT):
        ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0,rounding_size=0.09", fc=CARD, ec=ec, lw=1.4))
        ax.text(x + 0.18, y + h - 0.3, title, color=tc, fontsize=11.5, fontweight="medium", va="center")
        for i, l in enumerate(lines): ax.text(x + 0.18, y + h - 0.62 - 0.27 * i, l, color=SUB, fontsize=9.3, va="center")
    ax.text(0.4, 4.2, "The system at a glance", color=TXT, fontsize=17, fontweight="medium"); ax.text(0.4, 3.87, "AUP-ZU3 board (Zynq UltraScale+ xczu3eg). The atoms meet only on one register bus and one memory port.", color=SUB, fontsize=10.5)
    box(0.4, 2.05, 3.0, 1.45, "Cortex-A53", ("Rust job-graph scheduler", "lays out calls, picks an atom,", "sets width and rounds"), ec=GOLD)
    box(0.4, 0.35, 3.0, 1.35, "DDR memory", ("tenant buffers, read in place", "no copy between processor", "and atoms"))
    # bus and port
    ax.add_patch(Rectangle((4.15, 0.35), 0.16, 3.15, fc=GOLD, ec="none")); ax.text(4.23, 3.62, "register bus", color=GOLD, fontsize=9.3, ha="center")
    ax.add_patch(Rectangle((5.0, 0.35), 0.16, 3.15, fc=BLUE, ec="none")); ax.text(5.08, 3.62, "memory port", color=BLUE, fontsize=9.3, ha="center"); ax.text(5.08, 0.13, "64 bit, 19.2 Gb/s", color=SUB, fontsize=8.8, ha="center")
    ax.add_patch(FancyArrowPatch((3.4, 2.75), (4.15, 2.75), arrowstyle="-|>", mutation_scale=13, color=GOLD, lw=1.6)); ax.add_patch(FancyArrowPatch((3.4, 1.0), (5.0, 1.0), arrowstyle="<|-|>", mutation_scale=13, color=BLUE, lw=1.6))
    # atoms
    def atom(x, y, splitm, label):
        ax.add_patch(FancyBboxPatch((x, y), 2.9, 0.62, boxstyle="round,pad=0,rounding_size=0.07", fc=CARD, ec=EDGE, lw=1.3))
        c = 0.085
        for r in range(5):
            for q in range(5):
                X, Y = x + 0.14 + q * (c + 0.016), y + 0.06 + r * (c + 0.016)
                if splitm: ax.add_patch(Rectangle((X, Y), c / 2 - 0.004, c, fc=GOLD, ec="none")); ax.add_patch(Rectangle((X + c / 2 + 0.004, Y), c / 2 - 0.004, c, fc=BLUE, ec="none"))
                else: ax.add_patch(Rectangle((X, Y), c, c, fc=GOLD, ec="none"))
        ax.text(x + 0.82, y + 0.31, label, color=TXT, fontsize=9.8, va="center")
        ax.add_patch(FancyArrowPatch((4.31, y + 0.42), (x, y + 0.42), arrowstyle="-|>", mutation_scale=10, color=GOLD, lw=1.2)); ax.add_patch(FancyArrowPatch((5.16, y + 0.2), (x, y + 0.2), arrowstyle="<|-|>", mutation_scale=10, color=BLUE, lw=1.2))
    atom(6.0, 2.88, False, "atom 0   1 × 1600   SHAKE256")
    atom(6.0, 2.08, False, "atom 1   1 × 1600   TurboSHAKE128")
    atom(6.0, 1.28, True, "atom 2   2 × 800   MiniSHAKE128")
    atom(6.0, 0.48, True, "atom 3   2 × 800   MiniSHAKE128")
    ax.text(9.25, 3.3, "Each atom", color=TXT, fontsize=11.5, fontweight="medium");
    for i, l in enumerate(("one Keccak round per clock", "400 to 480 MHz", "hash chains stay in hardware", "bus master inside its own", "address window", "width and rounds set per job")):
        ax.text(9.25, 2.98 - 0.29 * i, l, color=SUB, fontsize=9.6)
    ax.text(9.25, 0.9, "Software groups atoms per tenant", color=TXT, fontsize=10); ax.text(9.25, 0.62, "and re-forms them between jobs.", color=TXT, fontsize=10)
    save(fig, "system.png")

if __name__ == "__main__":
    stats(); split(); compare(); amdahl(); system()
