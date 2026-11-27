#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""README figures, part 2: the material the four-page letter had no room to draw.
All values measured on aup1 (PAPER_FACTS.md, SIGN_RESULT.md, MODES_RESULT.md, RQ1_RESULT.md) or read from the filed forecast files.
Colour follows the entity everywhere: MiniSHAKE gold, TurboSHAKE blue, SHAKE aqua; other work neutral grey; status green/red with a mark and a word."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager as fm, rcParams
from matplotlib.patches import FancyBboxPatch, Rectangle, FancyArrowPatch
import numpy as np

G = "/usr/share/fonts/truetype/google-fonts/ofl/"
for f in ("firasans/FiraSans-Regular.ttf", "firasans/FiraSans-Medium.ttf", "firasans/FiraSans-Light.ttf", "firasans/FiraSans-SemiBold.ttf"):
    fm.fontManager.addfont(G + f)
rcParams["font.family"] = "Fira Sans"; rcParams["mathtext.fontset"] = "dejavusans"
BG = "#0b1a12"; CARD = "#10241a"; EDGE = "#1d3a2b"; TXT = "#ece2c8"; SUB = "#9aa69f"; GRID = "#1a3024"
GOLD = "#c2932f"; BLUE = "#338ced"; AQUA = "#00a888"; GREY = "#7d8a83"; GOOD = "#0aac23"; BAD = "#eb6263"
OUT = "docs/img/"

def save(fig, name):
    fig.savefig(OUT + name, dpi=200, facecolor=BG, bbox_inches="tight", pad_inches=0.18); plt.close(fig); print(name)
def axstyle(ax, title, sub=None):
    ax.set_facecolor(CARD)
    for s in ax.spines.values(): s.set_color(EDGE); s.set_linewidth(0.8)
    ax.tick_params(colors=SUB, labelsize=9, length=0, pad=5); ax.grid(color=GRID, lw=0.8); ax.set_axisbelow(True)
    ax.set_title(title, color=TXT, fontsize=12.5, fontweight="medium", loc="left", pad=22 if sub else 10)
    if sub: ax.text(0, 1.035, sub, transform=ax.transAxes, color=SUB, fontsize=9, va="bottom")
def canvas(w, h):
    fig = plt.figure(figsize=(w, h)); fig.patch.set_facecolor(BG); ax = fig.add_axes([0, 0, 1, 1]); ax.axis("off"); ax.set_xlim(0, w); ax.set_ylim(0, h); return fig, ax
def box(ax, x, y, w, h, title, lines=(), ec=EDGE, tc=TXT, fs=11.5):
    ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0,rounding_size=0.09", fc=CARD, ec=ec, lw=1.5))
    ax.text(x + 0.18, y + h - 0.3, title, color=tc, fontsize=fs, fontweight="medium", va="center")
    for i, l in enumerate(lines): ax.text(x + 0.18, y + h - 0.62 - 0.27 * i, l, color=SUB, fontsize=9.3, va="center")
def arrow(ax, a, b, col=TXT, style="-|>", lw=1.6):
    ax.add_patch(FancyArrowPatch(a, b, arrowstyle=style, mutation_scale=14, color=col, lw=lw, shrinkA=0, shrinkB=0))

# ---------------------------------------------------------------- design method
def method():
    fig, ax = canvas(12, 4.9)
    ax.text(0.4, 4.5, "Forecast first, measure second", color=TXT, fontsize=17, fontweight="medium")
    ax.text(0.4, 4.17, "The model settled the hardware and the software before either was finished. Every forecast was filed before its run.", color=SUB, fontsize=10.5)
    box(ax, 0.4, 2.4, 2.55, 1.4, "gem5", ("Cortex-A53 model", "cost of each scheduler", "routine"), ec=BLUE, tc=BLUE)
    box(ax, 0.4, 0.65, 2.55, 1.4, "The board", ("atom time per job", "memory-port rate", "cache-flush cost"), ec=GOLD, tc=GOLD)
    box(ax, 3.75, 1.55, 2.3, 1.35, "Event simulator", ("one processor core,", "N atoms, one port", "nothing fitted"))
    arrow(ax, (2.95, 3.05), (3.75, 2.5), BLUE); arrow(ax, (2.95, 1.4), (3.75, 1.95), GOLD)
    box(ax, 6.85, 2.75, 2.6, 1.15, "Hardware: how many atoms?", ("atom sweep from 1 to 16", "the device holds four"))
    box(ax, 6.85, 0.55, 2.6, 1.15, "Software: which scheduler?", ("13 designs ranked", "reading in place ranked first"))
    arrow(ax, (6.05, 2.5), (6.85, 3.2)); arrow(ax, (6.05, 1.95), (6.85, 1.25))
    box(ax, 10.05, 2.75, 1.6, 1.15, "Forecast", ("filed with a", "failure condition"), ec=AQUA, tc=AQUA)
    box(ax, 10.05, 0.55, 1.6, 1.15, "Board run", ("n = 25,", "every output checked"), ec=GOLD, tc=GOLD)
    arrow(ax, (9.45, 3.32), (10.05, 3.32)); arrow(ax, (9.45, 1.12), (10.05, 1.12)); arrow(ax, (10.85, 2.75), (10.85, 1.7), AQUA)
    ax.text(11.0, 2.22, "then", color=SUB, fontsize=9.5)
    ax.text(3.75, 0.62, "After the runs: 46 board runs reproduced", color=TXT, fontsize=9.6); ax.text(3.75, 0.36, "to 4.9% mean and 9.5% worst.", color=TXT, fontsize=9.6)
    ax.text(6.85, 2.2, "The model resolves choices worth more than 10%.", color=SUB, fontsize=9.6)
    save(fig, "method.png")

# ---------------------------------------------------------------- cost of the split + chain step
def cost():
    fig, axs = plt.subplots(1, 3, figsize=(13.2, 4.3), gridspec_kw={"width_ratios": [1.15, 0.9, 1.2]}); fig.patch.set_facecolor(BG); fig.subplots_adjust(left=0.115, right=0.985, top=0.76, bottom=0.14, wspace=0.55)
    ax = axs[0]; axstyle(ax, "What the second width costs", "LUTs of the atom block, one-atom system, post-route")
    rows = [("Fixed, 2 × 800 only", 7421, GREY), ("Fixed, 1 × 1600 only", 7419, GREY), ("Splittable (RUTY-ATOM)", 8064, GOLD)]
    for i, (n, v, col) in enumerate(rows):
        ax.barh(i, v, height=0.5, color=col, ec="none"); ax.text(v + 120, i, "{:,}".format(v), color=TXT, fontsize=10, va="center")
    ax.plot([7419, 7419], [1.7, 2.3], color=TXT, lw=1.0, ls=(0, (3, 3))); ax.text(7419 - 150, 2.42, "+645 LUTs, +8.7%", color=TXT, fontsize=9.6, ha="right", va="center")
    ax.set_yticks(range(3)); ax.set_yticklabels([r[0] for r in rows], fontsize=9.5)
    for t, r in zip(ax.get_yticklabels(), rows): t.set_color(TXT if r[2] == GOLD else SUB)
    ax.set_xlim(0, 9800); ax.set_ylim(-0.6, 2.75); ax.grid(axis="y", lw=0); ax.set_xticks([0, 2000, 4000, 6000, 8000]); ax.set_xticklabels(["0", "2k", "4k", "6k", "8k"])
    ax = axs[1]; ax.set_facecolor(BG); ax.axis("off"); ax.set_xlim(0, 1); ax.set_ylim(0, 1)
    ax.text(0, 1.115, "And what it does not", color=TXT, fontsize=12.5, fontweight="medium", va="bottom")
    for i, (big, small) in enumerate((("+3", "flip-flops (4,397 → 4,400)"), ("480 MHz", "met by all three builds"), ("0", "mismatches on the board, all three"), ("754", "one-bit multiplexers do it all"))):
        y = 0.86 - i * 0.26; ax.add_patch(Rectangle((0, y - 0.085), 0.018, 0.17, fc=GOLD, ec="none"))
        ax.text(0.06, y + 0.03, big, color=TXT, fontsize=17, fontweight="semibold", va="center"); ax.text(0.06, y - 0.065, small, color=SUB, fontsize=9.4, va="center")
    ax = axs[2]; axstyle(ax, "One step of a hash chain", "nanoseconds per chain, in the atom at 450 MHz (lower is better)")
    rows = [("SHAKE256\n24 rounds", 59.7, AQUA), ("TurboSHAKE256\n12 rounds", 33.1, BLUE), ("MiniSHAKE128\ntwo at once", 18.4, GOLD)]
    for i, (n, v, col) in enumerate(rows):
        ax.bar(i, v, width=0.5, color=col, ec="none"); ax.text(i, v + 1.6, "%.1f" % v, color=TXT, fontsize=10.5, ha="center")
    for i in (0, 1):
        ax.annotate("", xy=(i + 0.78, rows[i + 1][1] + 9), xytext=(i + 0.22, rows[i][1] + 2 if i else rows[i][1] - 6), arrowprops=dict(arrowstyle="-|>", color=TXT, lw=1.2, connectionstyle="arc3,rad=-0.25"))
        ax.text(i + 0.62, (rows[i][1] + rows[i + 1][1]) / 2 + 11, "1.80×", color=TXT, fontsize=10, ha="center")
    ax.set_xticks(range(3)); ax.set_xticklabels([r[0] for r in rows], fontsize=9.2, color=TXT); ax.set_ylim(0, 72); ax.set_xlim(-0.55, 2.55); ax.grid(axis="x", lw=0)
    fig.text(0.115, 0.925, "The cost, and the gain inside the core", color=TXT, fontsize=16, fontweight="medium")
    save(fig, "cost.png")

# ---------------------------------------------------------------- a full signature across atoms, and where its time goes
def signature():
    fig, axs = plt.subplots(1, 2, figsize=(12.6, 4.4), gridspec_kw={"width_ratios": [1, 1.15]}); fig.patch.set_facecolor(BG); fig.subplots_adjust(left=0.07, right=0.985, top=0.76, bottom=0.14, wspace=0.42)
    ax = axs[0]; axstyle(ax, "A complete SLH-DSA-128f signature", "median time to sign, ms (lower is better)")
    N = [1, 2, 4]
    S = (("SHAKE256 (the FIPS 205 signature)", [9.78, 5.56, 3.68], AQUA, "D"), ("TurboSHAKE256", [6.81, 3.97, 3.00], BLUE, "s"), ("MiniSHAKE128, split state", [5.41, 3.51, 3.12], GOLD, "o"))
    for lab, y, col, mk in S: ax.plot(N, y, "-", color=col, lw=2.2, marker=mk, ms=8, mec=CARD, mew=1.6, label=lab, zorder=3)
    ax.text(1.0, 10.35, "9.78", color=TXT, fontsize=9.5, ha="center"); ax.text(1.0, 4.55, "5.41", color=TXT, fontsize=9.5, ha="center"); ax.text(4.0, 4.25, "3.68", color=TXT, fontsize=9.5, ha="center"); ax.text(4.0, 2.2, "3.00", color=TXT, fontsize=9.5, ha="center")
    ax.set_xticks(N); ax.set_xlim(0.75, 4.25); ax.set_ylim(0, 11.5); ax.set_xlabel("atoms", color=SUB, fontsize=9.5, labelpad=4)
    ax.legend(loc="upper right", fontsize=9, frameon=False, labelcolor=TXT, handlelength=2.2, labelspacing=0.4)
    ax = axs[1]; axstyle(ax, "Where the time goes on four atoms", "ms per signature: processor laying out calls, then atoms and memory port")
    rows = [("SHAKE256", 2.27, 3.68, AQUA), ("TurboSHAKE256", 2.28, 3.00, BLUE), ("MiniSHAKE128, split", 2.65, 3.12, GOLD)]
    for i, (n, p, t, col) in enumerate(rows[::-1]):
        ax.barh(i, p, height=0.5, color=GREY, ec="none"); ax.barh(i, t - p - 0.03, left=p + 0.03, height=0.5, color=col, ec="none")
        ax.text(p / 2, i, "processor %.2f" % p, color=BG, fontsize=9.3, ha="center", va="center", fontweight="medium"); ax.text(t + 0.07, i, "%.2f" % t, color=TXT, fontsize=10, va="center")
    ax.set_yticks(range(3)); ax.set_yticklabels([r[0] for r in rows[::-1]], fontsize=9.6, color=TXT); ax.set_xlim(0, 4.3); ax.set_ylim(-0.6, 2.6); ax.grid(axis="y", lw=0)
    ax.text(0.0, -0.21, "The atoms divide their share of the work; the one processor core does not. That is why the split, which only speeds up the atoms, gains nothing here.", transform=ax.transAxes, color=SUB, fontsize=8.9, wrap=True)
    fig.text(0.07, 0.925, "The whole application, not the kernel", color=TXT, fontsize=16, fontweight="medium")
    save(fig, "signature.png")

# ---------------------------------------------------------------- forecast scorecard
def forecasts():
    rows = [("Area of the second width", "7 to 12% more LUTs", "8.7%", True),
            ("Standard signature, sign", "9.9 / 5.6 / 3.3 ms, within 20%", "9.70 / 5.66 / 3.70 ms", True),
            ("Standard signature, key generation", "0.34 / 0.19 / 0.12 ms", "0.39 / 0.27 / 0.20 ms", False),
            ("Standard signature, verification", "0.75 / 0.55 / 0.45 ms", "1.05 / 1.01 / 1.25 ms", False),
            ("Split signature after the layout fix", "4.3 / 2.6 / 1.8 ms, within 25%", "5.41 / 3.53 / 3.13 ms", False),
            ("Scheduler that reads in place", "within 10% of the model", "9 of 10 jobs within 10%", True),
            ("Two tenants, full applications", "at most 8% slower when shared", "at most 1.7%", True),
            ("KangarooTwelve, 16 MB", "7.7 / 7.05 / 7.05 ms", "7.68 / 7.04 / 7.04 ms", True),
            ("Merkle tree, nine times", "each within 25%", "all within 7%", True),
            ("Split over wide, from Amdahl's law", "1.25 / 1.10 / 1.00×", "1.35 / 1.16 / 1.01×", True),
            ("Cortex-A53 scaling, four cores", "3.6 to 3.9×", "3.99× (better than forecast)", False)]
    fig, ax = canvas(12, 0.42 * len(rows) + 1.75)
    H = 0.42 * len(rows) + 1.75
    ax.text(0.4, H - 0.42, "Every forecast, including the ones that failed", color=TXT, fontsize=17, fontweight="medium")
    ax.text(0.4, H - 0.75, "Filed before the run, with a failure condition. They held where atoms, port or area set the answer, and failed where an unmeasured processor cost did.", color=SUB, fontsize=10)
    y0 = H - 1.22
    for x, t in ((0.5, "What was forecast"), (4.55, "Forecast"), (7.75, "Measured"), (10.75, "Result")): ax.text(x, y0, t, color=SUB, fontsize=9.6, fontweight="medium")
    ax.plot([0.4, 11.6], [y0 - 0.16, y0 - 0.16], color=EDGE, lw=1.0)
    for i, (what, fc, ms, ok) in enumerate(rows):
        y = y0 - 0.45 - i * 0.42
        if i % 2 == 0: ax.add_patch(Rectangle((0.4, y - 0.2), 11.2, 0.42, fc=CARD, ec="none"))
        ax.text(0.5, y, what, color=TXT, fontsize=10.2, va="center"); ax.text(4.55, y, fc, color=SUB, fontsize=9.8, va="center"); ax.text(7.75, y, ms, color=TXT, fontsize=9.8, va="center")
        col = GOOD if ok else BAD
        ax.text(10.75, y, ("✓  held" if ok else "✕  missed"), color=col, fontsize=10.2, va="center", fontweight="medium", fontfamily="DejaVu Sans")
    save(fig, "forecasts.png")

if __name__ == "__main__":
    method(); cost(); signature(); forecasts()
