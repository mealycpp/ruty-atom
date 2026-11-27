#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""fig_sw: the software side of RUTY-ATOM. (a) job graph of one SLH-DSA-128f signature, nodes drawn as atoms in the
shape they run on; (b) the scheduler loop and the two word layouts; (c) where the sign time goes at four atoms."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle, Circle, FancyArrowPatch, Wedge
from matplotlib import rcParams
rcParams["font.family"] = "STIXGeneral"; rcParams["mathtext.fontset"] = "stix"
rcParams["pdf.fonttype"] = 42; rcParams["hatch.linewidth"] = 0.4
W, H = 7.16, 2.55
K = "black"; B = "#1f4e9c"; G = "#f0f0f0"; FS = 6.6
Y0 = 0.52
fig = plt.figure(figsize=(W, H - Y0)); ax = fig.add_axes([0, 0, 1, 1]); ax.set_xlim(0, W); ax.set_ylim(Y0, H); ax.axis("off")

def txt(x, y, t, c=K, fs=FS, ha="center", va="center", **kw):
    ax.text(x, y, t, color=c, fontsize=fs, ha=ha, va=va, zorder=8, **kw)
def arr(p, q, lw=0.6, c=K, z=3, cs=None, ms=5):
    ax.add_patch(FancyArrowPatch(p, q, arrowstyle="-|>", mutation_scale=ms, lw=lw, color=c, zorder=z,
                                 shrinkA=0, shrinkB=0, connectionstyle=cs or "arc3"))
def line(pts, lw=0.6, c=K, z=3, ls="-"):
    ax.plot([p[0] for p in pts], [p[1] for p in pts], color=c, lw=lw, zorder=z, ls=ls, solid_capstyle="butt")
R = 0.075
def node(x, y, split=False, z=5, ghost=False):
    ec = "0.6" if ghost else K
    ax.add_patch(Circle((x, y), R, fc="white", ec=ec, lw=0.9 if not ghost else 0.6, zorder=z))
    if split and not ghost:
        ax.plot([x, x], [y - R, y + R], color=B, lw=1.0, ls=(0, (2, 1.2)), zorder=z + 1)
def box(x, y, w, h, t=None, fc="white", ec=K, lw=0.8, fs=FS, **kw):
    ax.add_patch(Rectangle((x, y), w, h, fc=fc, ec=ec, lw=lw, zorder=3))
    if t: txt(x + w / 2, y + h / 2, t, fs=fs, **kw)

# titles
txt(0.04, 2.45, "(a) Job graph of one signature", ha="left", fontweight="bold", fs=7.2)
txt(3.74, 2.45, "(b) Scheduler", ha="left", fontweight="bold", fs=7.2)
txt(5.52, 2.45, "(c) Sign time, four atoms", ha="left", fontweight="bold", fs=7.2)

# ============================== (a) job graph
def row(y, xs, splits, labels, mult, name, ghost_n=2):
    for g in range(ghost_n, 0, -1):                       # stacked copies behind
        for i, x in enumerate(xs):
            node(x + 0.035 * g, y + 0.035 * g, ghost=True, z=2)
    for i, x in enumerate(xs):
        if i: arr((xs[i - 1] + R, y), (x - R, y))
        node(x, y, splits[i])
        txt(x, y - 0.155, labels[i], fs=5.6)
    txt(0.66, y + 0.05, name, ha="right", fs=6.4, fontweight="bold")
    txt(0.66, y - 0.085, mult, ha="right", fs=5.8)
    arr((0.80, y), (xs[0] - R, y))
XS = [0.98, 1.33, 1.68, 2.03, 2.38, 2.73]
YF, YT = 1.62, 0.78
# message hashes
node(0.50, 2.10); txt(0.50, 2.255, "PRF", fs=5.6)
arr((0.50 + R, 2.10), (0.80 - R, 2.10)); node(0.80, 2.10); txt(0.80, 2.255, "H", fs=5.6)
txt(0.04, 2.10, "message", fs=6.4, fontweight="bold", ha="left")
# FORS
row(YF, XS[:5], [True, True, False, False, False], ["PRF", "F", "H", "H …", r"T$_k$"], r"$\times$33", "FORS")
# XMSS trees
row(YT, XS, [True, True, False, False, False, False], ["PRF", "F chain", r"T$_{len}$", "H", "H", "H"], r"$\times$22", "XMSS")
# chain stays in the atom
ax.add_patch(FancyArrowPatch((XS[1] - 0.05, YT + R + 0.01), (XS[1] + 0.05, YT + R + 0.01), arrowstyle="-|>", mutation_scale=4.5,
                             lw=0.7, color=K, zorder=6, connectionstyle="arc3,rad=-1.6"))
txt(XS[1] + 0.02, YT + 0.30, r"$\times$15, inside the atom", fs=5.6, ha="left", style="italic")
# index from H(message) to both families
line([(0.80, 2.10 - R), (0.80, YT)], lw=0.6)
txt(0.765, 1.20, "tree index", fs=5.6, rotation=90, ha="right", style="italic", c="0.25")
# WOTS signatures and output
XW = 3.14; YW = 1.20
for g in (2, 1): node(XW + 0.035 * g, YW + 0.035 * g, ghost=True, z=2)
node(XW, YW, True); txt(XW + 0.10, YW - 0.16, "F sign", fs=5.6, ha="left"); txt(XW + 0.02, YW + 0.21, r"$\times$22", fs=5.8)
arr((XS[4] + R, YF), (XW - 0.055, YW + 0.055), cs="arc3,rad=-0.12")
arr((XS[5] + R, YT), (XW - 0.055, YW - 0.055), cs="arc3,rad=0.12")
arr((XW + R, YW), (3.40, YW)); txt(3.42, YW, "sig", fs=6.4, ha="left", fontweight="bold")
# legend
node(1.45, 2.20); txt(1.56, 2.20, r"runs on 1$\times$1600", ha="left", fs=5.8)
node(1.45, 1.98, True); txt(1.56, 1.98, r"runs on 2$\times$800 (two jobs per atom)", ha="left", fs=5.8, c=B)

# ============================== (b) scheduler loop
line([(3.62, 0.58), (3.62, 2.36)], lw=0.5, c="0.6"); line([(5.42, 0.58), (5.42, 2.36)], lw=0.5, c="0.6")
bx, bw, bh = 3.80, 1.44, 0.24
ys = [2.02, 1.68, 1.10, 0.74]
box(bx, ys[0], bw, bh, "ready nodes", fc=G)
box(bx, ys[1], bw, bh, "pick: atom, shape, rounds")
box(bx, ys[2], bw, 0.50, None)
txt(bx + bw / 2, ys[2] + 0.42, "pack words (no copy)", fs=6.6)
box(bx, ys[3], bw, bh, "5 register writes → atom")
for i in range(3):
    top = ys[i]; nxt = ys[i + 1] + (0.50 if i == 1 else bh)
    arr((bx + bw / 2, top), (bx + bw / 2, nxt))
# done -> release successors
line([(bx + bw, ys[3] + bh / 2), (bx + bw + 0.10, ys[3] + bh / 2), (bx + bw + 0.10, ys[0] + bh / 2)], lw=0.6)
arr((bx + bw + 0.10, ys[0] + bh / 2), (bx + bw, ys[0] + bh / 2))
txt(bx + bw + 0.135, 1.42, "done: release successors", fs=5.6, rotation=90, style="italic")
# word layouts
wy1, wy2, wx, ww, wh = ys[2] + 0.20, ys[2] + 0.045, bx + 0.42, 0.92, 0.11
box(wx, wy1, ww, wh, None, lw=0.7)
txt(wx - 0.04, wy1 + wh / 2, "wide", fs=5.6, ha="right"); txt(wx + ww / 2, wy1 + wh / 2, "8 bytes of job a", fs=5.4)
box(wx, wy2, ww, wh, None, lw=0.7)
ax.plot([wx + ww / 2, wx + ww / 2], [wy2, wy2 + wh], color=B, lw=1.0, ls=(0, (2, 1.2)), zorder=5)
txt(wx - 0.04, wy2 + wh / 2, "split", fs=5.6, ha="right", c=B)
txt(wx + ww * 0.25, wy2 + wh / 2, "4 of b", fs=5.4, c=B); txt(wx + ww * 0.75, wy2 + wh / 2, "4 of a", fs=5.4, c=B)

# ============================== (c) where the time goes
import json, os
D = [("SHAKE256", 3.68, 2.27, False), ("TurboSHAKE256", 3.00, 2.28, False), ("MiniSHAKE128", 3.12, 2.65, True)]
if os.path.exists("sign_bars.json"):
    D = [tuple(r) for r in json.load(open("sign_bars.json"))]
x0, x1 = 5.58, 7.06; sc = (x1 - x0) / 4.4
yb = [1.92, 1.50, 1.08]; hb = 0.18
for (name, tot, proc, sp), y in zip(D, yb):
    c = B if sp else K
    ax.add_patch(Rectangle((x0, y), proc * sc, hb, fc="white", ec=c, lw=0.8, hatch="/////", zorder=3))
    ax.add_patch(Rectangle((x0 + proc * sc, y), (tot - proc) * sc, hb, fc="white", ec=c, lw=0.8, zorder=3))
    txt(x0, y + hb + 0.075, name + (" (split)" if sp else ""), ha="left", fs=6.2, c=c)
    txt(x1, y + hb + 0.075, "%.2f ms" % tot, ha="right", fs=6.0, c=c)
line([(x0, 0.96), (x1, 0.96)], lw=0.6)
for t in (0, 1, 2, 3, 4):
    line([(x0 + t * sc, 0.96), (x0 + t * sc, 0.93)], lw=0.6); txt(x0 + t * sc, 0.86, str(t), fs=5.8)
txt(x1 + 0.04, 0.86, "ms", fs=5.8, ha="left")
ax.add_patch(Rectangle((x0, 0.62), 0.14, 0.09, fc="white", ec=K, lw=0.7, hatch="/////", zorder=3))
txt(x0 + 0.17, 0.665, "processor", ha="left", fs=5.8)
ax.add_patch(Rectangle((x0 + 0.72, 0.62), 0.14, 0.09, fc="white", ec=K, lw=0.7, zorder=3))
txt(x0 + 0.89, 0.665, "atoms, port", ha="left", fs=5.8)
fig.savefig("fig_sw.pdf", bbox_inches="tight", pad_inches=0.01); fig.savefig("fig_sw.png", dpi=300, bbox_inches="tight", pad_inches=0.01); print("ok")
