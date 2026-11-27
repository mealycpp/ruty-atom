#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""fig_arch3: RUTY-ATOM architecture as a schematic (system | atom shell | core).
Drawn from ruty_top3.v, atom_seq3.v, build_atom3.tcl (see ARCH_NOTES.md)."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle, Polygon, Circle, FancyBboxPatch
from matplotlib import rcParams

rcParams["font.family"] = "STIXGeneral"
rcParams["mathtext.fontset"] = "stix"
rcParams["pdf.fonttype"] = 42
rcParams["hatch.linewidth"] = 0.4

W, H = 7.16, 2.95
K = "black"
B = "#1f4e9c"      # accent: everything that depends on dual
G = "#f0f0f0"
FS = 6.6
LT, LB = 0.6, 1.35  # thin wire, bus

fig = plt.figure(figsize=(W, H))
ax = fig.add_axes([0, 0, 1, 1])
ax.set_xlim(0, W); ax.set_ylim(0, H); ax.axis("off")


def box(x, y, w, h, t=None, fc="white", ec=K, lw=0.8, fs=FS, z=3, **kw):
    ax.add_patch(Rectangle((x, y), w, h, fc=fc, ec=ec, lw=lw, zorder=z))
    if t:
        ax.text(x + w / 2, y + h / 2, t, ha="center", va="center", fontsize=fs, zorder=z + 1, **kw)


def reg(x, y, w, h, t=None, ec=K, lw=0.9, fs=FS, clk="bl", **kw):
    """register: box with clock triangle on the bottom edge"""
    box(x, y, w, h, t, ec=ec, lw=lw, fs=fs, **kw)
    cx = x + 0.07 if clk == "bl" else x + w - 0.07
    ax.add_patch(Polygon([(cx - 0.035, y), (cx, y + 0.05), (cx + 0.035, y)], closed=False,
                         fc="none", ec=ec, lw=0.6, zorder=5))


def wire(pts, lw=LT, c=K, arrow=False, z=2, ls="-"):
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
    ax.plot(xs, ys, color=c, lw=lw, solid_capstyle="butt", solid_joinstyle="miter", zorder=z, ls=ls)
    if arrow:
        (x0, y0), (x1, y1) = pts[-2], pts[-1]
        ax.annotate("", xy=(x1, y1), xytext=(x0, y0), zorder=z,
                    arrowprops=dict(arrowstyle="-|>", color=c, lw=lw, mutation_scale=4.5 + 1.2 * lw,
                                    shrinkA=0, shrinkB=0))


def dot(x, y, c=K, r=0.018):
    ax.add_patch(Circle((x, y), r, fc=c, ec=c, lw=0, zorder=6))


def slash(x, y, t, c=K, dy=0.075, vert=False, fs=5.8, dx=0.0):
    if vert:
        ax.plot([x - 0.035, x + 0.035], [y - 0.03, y + 0.03], color=c, lw=0.6, zorder=6)
        ax.text(x + 0.05 + dx, y, t, ha="left", va="center", fontsize=fs, color=c, zorder=7)
    else:
        ax.plot([x - 0.03, x + 0.03], [y - 0.035, y + 0.035], color=c, lw=0.6, zorder=6)
        ax.text(x + dx, y + dy, t, ha="center", va="center", fontsize=fs, color=c, zorder=7)


def xor(x, y, r=0.055):
    ax.add_patch(Circle((x, y), r, fc="white", ec=K, lw=0.8, zorder=5))
    ax.plot([x - r, x + r], [y, y], color=K, lw=0.6, zorder=6)
    ax.plot([x, x], [y - r, y + r], color=K, lw=0.6, zorder=6)


def mux_r(x, y, w, h, c=K, lw=0.8):
    """mux, inputs on the left (tall side), output on the right"""
    ax.add_patch(Polygon([(x, y - h / 2), (x, y + h / 2), (x + w, y + h / 2 - 0.3 * h), (x + w, y - h / 2 + 0.3 * h)],
                         closed=True, fc="white", ec=c, lw=lw, zorder=4))


def mux_d(x, y, w, h, c=B, lw=0.8):
    """mux, inputs on top (wide side), output at the bottom; (x, y) = centre"""
    ax.add_patch(Polygon([(x - w / 2, y + h / 2), (x + w / 2, y + h / 2), (x + 0.2 * w, y - h / 2), (x - 0.2 * w, y - h / 2)],
                         closed=True, fc="white", ec=c, lw=lw, zorder=4))


def mux_u(x, y, w, h, c=B, lw=0.8):
    ax.add_patch(Polygon([(x - w / 2, y - h / 2), (x + w / 2, y - h / 2), (x + 0.2 * w, y + h / 2), (x - 0.2 * w, y + h / 2)],
                         closed=True, fc="white", ec=c, lw=lw, zorder=4))


def fifo(x, y, w, h):
    box(x, y, w, h, lw=0.8)
    n = 4
    for i in range(1, n):
        ax.plot([x + w * i / n, x + w * i / n], [y, y + h], color=K, lw=0.5, zorder=4)


def sync(x, y, n=2, s=0.075):
    for i in range(n):
        reg(x + i * (s + 0.012), y - s / 2, s, s, lw=0.6)


def txt(x, y, t, c=K, fs=FS, ha="center", va="center", **kw):
    ax.text(x, y, t, color=c, fontsize=fs, ha=ha, va=va, zorder=8, **kw)



# ---------------------------------------------------------------- panel frames
TOP = 2.72
X0, X1 = 2.16, 7.09
XC = 3.62                                   # clock-domain boundary
txt(0.04, 2.86, "(a) System", ha="left", fontweight="bold", fs=7.2)
txt(X0, 2.86, "(b) Atom shell", ha="left", fontweight="bold", fs=7.2)
txt(XC + 0.04, 2.86, "(c) Core", ha="left", fontweight="bold", fs=7.2)
txt(XC + 0.52, 2.86, "one Keccak round per clock", ha="left", fs=6.2, style="italic")
txt(X1, 2.86, "blue: set by dual", ha="right", fs=6.2, c=B, style="italic")
for d in (0.045, 0.022):
    ax.add_patch(Rectangle((X0 + d, 0.05 - d), X1 - X0, TOP - 0.02, fc="white", ec="0.45", lw=0.6, zorder=0))
ax.add_patch(Rectangle((X0, 0.05), X1 - X0, TOP - 0.02, fc="white", ec=K, lw=1.0, zorder=1))
txt(X1 - 0.04, 0.25, r"$\times N$", ha="right", fs=6.6)

# ================================================================ (a) SYSTEM
box(0.04, 1.38, 0.86, 1.34, lw=1.0, z=1)
txt(0.47, 2.645, "Cortex-A53", fs=6.8, fontweight="bold")
box(0.09, 1.69, 0.76, 0.89, fc=G, lw=0.6, z=2)
txt(0.47, 2.51, "scheduler", fs=6.2, style="italic")
gx = [0.20, 0.38, 0.56, 0.74]
R = 0.040


def node(x, y, s):
    if s == "d":
        ax.add_patch(Circle((x, y), R, fc=K, ec=K, lw=0.6, zorder=6))
    elif s == "r":
        ax.add_patch(Circle((x, y), R, fc="white", ec=K, lw=0.6, zorder=6, hatch="//////"))
    else:
        ax.add_patch(Circle((x, y), R, fc="white", ec=K, lw=0.6, zorder=6))


for y, st in ((2.38, "ddrw"), (2.24, "drww")):
    for i, s_ in enumerate(st):
        node(gx[i], y, s_)
        if i:
            wire([(gx[i - 1] + R, y), (gx[i] - R, y)], lw=0.5, arrow=True, z=5)
node(0.20, 2.10, "r"); node(0.38, 2.10, "w"); node(0.56, 2.10, "r")
for x, y, s_, t in ((0.17, 1.92, "d", "done"), (0.52, 1.92, "r", "running"), (0.17, 1.78, "w", "ready")):
    node(x, y, s_); txt(x + 0.06, y, t, ha="left", fs=5.6)
txt(0.86, 1.60, "HPM0", ha="right", fs=5.6)
txt(0.86, 1.46, "HP0", ha="right", fs=5.6)

box(0.04, 0.10, 0.86, 0.98, lw=1.0, z=1)
txt(0.47, 0.995, "DDR", fs=6.8, fontweight="bold")
box(0.12, 0.60, 0.70, 0.24, fc="white", lw=0.6, hatch="/////")
ax.add_patch(Rectangle((0.25, 0.655), 0.44, 0.13, fc="white", ec="none", zorder=4))
txt(0.47, 0.72, "buffer S", fs=6.0)
box(0.12, 0.22, 0.70, 0.24, lw=0.6)
txt(0.47, 0.34, "buffer T", fs=6.0)
wire([(0.72, 1.38), (0.72, 1.08)], lw=LB)
wire([(0.20, 1.69), (0.20, 0.84)], lw=0.5, ls=(0, (1, 1.5)), z=5, arrow=True)
txt(0.235, 1.23, "no copy", ha="left", fs=5.6, style="italic")

XR, XM = 1.04, 1.20
ay = [2.34, 1.76, 1.18, 0.60]
AX, AW, AH = 1.36, 0.50, 0.36
wire([(0.90, 1.60), (XR, 1.60)], lw=LT)
wire([(XR, ay[0] + 0.09), (XR, ay[3] + 0.09)], lw=LT)
wire([(0.90, 1.46), (XM, 1.46)], lw=LB)
wire([(XM, ay[0] - 0.09), (XM, ay[3] - 0.09)], lw=LB)
dot(XR, 1.60); dot(XM, 1.46, r=0.024)
for i, y in enumerate(ay):
    wire([(XR, y + 0.09), (AX, y + 0.09)], lw=LT, arrow=True)
    wire([(XM, y - 0.09), (AX, y - 0.09)], lw=LB)
    if 0 < i < 3:
        dot(XR, y + 0.09); dot(XM, y - 0.09, r=0.024)
txt(1.12, 0.28, "19.2 Gb/s", fs=5.6, rotation=90)

shapes = "wwss"
for i, y in enumerate(ay):
    reg(AX, y - AH / 2, AW, AH, "1600" if shapes[i] == "w" else None, fs=6.4, clk="br")
    if shapes[i] == "s":
        ax.plot([AX + AW / 2, AX + AW / 2], [y - AH / 2, y + AH / 2], color=B, lw=1.0, ls=(0, (2.5, 1.5)), zorder=5)
        txt(AX + AW * 0.25, y, "800", c=B, fs=6.4); txt(AX + AW * 0.75, y, "800", c=B, fs=6.4)
for (y0, y1, lab) in ((ay[1] - 0.24, ay[0] + 0.24, "S"), (ay[3] - 0.24, ay[2] + 0.24, "T")):
    ax.add_patch(FancyBboxPatch((AX - 0.06, y0), AW + 0.12, y1 - y0, boxstyle="round,pad=0,rounding_size=0.06",
                                fc="none", ec="0.35", lw=0.7, ls=(0, (4, 2)), zorder=2))
    txt(AX + AW + 0.085, (y0 + y1) / 2, lab, fs=6.6, fontweight="bold", ha="left")
txt(AX + AW / 2, 2.665, r"$N$ = 1, 2 or 4 atoms", fs=6.0, fontweight="bold")
txt(2.085, 1.50, "formations (software)", fs=5.6, rotation=90, style="italic", c="0.25")

# ================================================================ (b) ATOM SHELL
wire([(XC, 0.05), (XC, TOP + 0.03)], lw=0.6, c="0.4", ls=(0, (5, 2.5)), z=1)
txt(XC - 0.04, 2.66, "300 MHz", ha="right", fs=5.6, style="italic", c="0.3")
txt(XC + 0.04, 2.66, "400–450 MHz", ha="left", fs=5.6, style="italic", c="0.3")

reg(2.62, 2.20, 0.60, 0.32, "registers", fs=6.4)
wire([(X0, 2.36), (2.62, 2.36)], lw=LT, arrow=True)
txt(X0 + 0.04, 2.42, "s_axi", ha="left", fs=5.6)
YD = 2.44
wire([(3.22, YD), (4.30, YD)], lw=0.9, c=B)
txt(3.40, YD + 0.065, "dual", c=B, fs=6.2, fontweight="bold")

YI, YO, XT = 1.50, 0.42, 2.28
# data: memory -> input FIFO, output FIFO -> memory (no engine in the data path)
wire([(X0, 0.62), (XT, 0.62)], lw=LB)
wire([(XT, YO), (XT, YI)], lw=LB)
dot(XT, 0.62, r=0.024)
fifo(XC - 0.20, YI - 0.11, 0.40, 0.22)
wire([(XT, YI), (XC - 0.20, YI)], lw=LB, arrow=True)
fifo(XC - 0.20, YO - 0.11, 0.40, 0.22)
wire([(XC - 0.20, YO), (2.80, YO)], lw=LB, arrow=True)
wire([(2.80, YO), (XT, YO)], lw=LB)
slash(3.22, YI, "64"); slash(3.22, YO, "64")
txt(XC, YI + 0.18, "FIFO", fs=5.6); txt(XC, YO + 0.18, "FIFO", fs=5.6)
# the two address masters with the window check
MBX, MBW, MBY, MBH = 2.76, 0.36, 0.74, 0.44
box(MBX, MBY, MBW, MBH, "bus\nmasters", fs=6.2)
YAD = 1.10
wire([(MBX, YAD), (X0, YAD)], lw=LT, arrow=True)
txt(XT + 0.04, 0.53, "m_axi", ha="left", fs=5.6)
slash(2.63, YAD, "40", dy=0.07)
cx0, cyc = 2.52, 0.88                                  # window comparator
box(cx0, cyc - 0.12, 0.14, 0.24, None, lw=0.8)
txt(cx0 + 0.07, cyc, "cmp", fs=5.4, rotation=90)
wire([(cx0 + 0.14, cyc), (MBX, cyc)], lw=LT, arrow=True)
dot(2.415, YAD)
wire([(2.415, YAD), (2.415, cyc - 0.055), (cx0, cyc - 0.055)], lw=LT)          # address
wire([(2.74, 2.20), (2.74, 2.04), (2.47, 2.04), (2.47, cyc + 0.055), (cx0, cyc + 0.055)], lw=LT)   # window
txt(2.71, 2.10, "window", fs=5.6, ha="right")
wire([(2.94, 2.20), (2.94, MBY + MBH)], lw=LT, arrow=True)                    # batch settings
# go / idle between the masters and the sequencer
FX = 3.97
wire([(MBX + MBW, 1.08), (XC + 0.03, 1.08)], lw=LT, arrow=True)
sync(XC + 0.03, 1.08, 3, s=0.07)
wire([(XC + 0.03 + 3 * 0.082 - 0.012, 1.08), (FX, 1.08)], lw=LT, arrow=True)
txt(3.36, 1.14, "go", fs=5.6)
sync(XC - 0.03 - 2 * 0.087 + 0.012, 0.82, 2)
wire([(FX, 0.82), (XC - 0.03, 0.82)], lw=LT, arrow=True)
wire([(XC - 0.03 - 2 * 0.087 + 0.012, 0.82), (MBX + MBW, 0.82)], lw=LT, arrow=True)
txt(3.28, 0.755, "idle", fs=5.6)

# ================================================================ (c) CORE
YA = 1.50
box(FX, 0.70, 0.24, 0.50, "FSM", fs=6.2)
box(3.92, YA - 0.13, 0.30, 0.26, "pad", fs=6.4, ec=B, lw=1.0)
wire([(XC + 0.20, YA), (3.92, YA)], lw=LB, arrow=True)
XX = 4.35
xor(XX, YA)
wire([(4.22, YA), (XX - 0.055, YA)], lw=LB)
wire([(XX, YA + 0.20), (XX, YA + 0.055)], lw=LB, arrow=True)
txt(XX, YA + 0.255, r"$A_i$", fs=6.0)
MX = 4.52
mux_r(MX, YA, 0.11, 0.62)
wire([(XX + 0.055, YA), (MX, YA)], lw=LB)
AXc, AWc, AHc = 4.76, 0.62, 1.00
reg(AXc, YA - AHc / 2, AWc, AHc, None, lw=1.3)
wire([(MX + 0.11, YA), (AXc, YA)], lw=LB, arrow=True)
ax.plot([AXc, AXc + AWc], [YA, YA], color=B, lw=1.1, ls=(0, (2.5, 1.5)), zorder=5)
txt(AXc + AWc / 2, YA + 0.37, r"$\mathbf{A}$", fs=7.6)
txt(AXc + AWc / 2, YA + 0.21, "25 lanes", fs=5.8)
txt(AXc + AWc / 2, YA + 0.075, "bits 63…32", c=B, fs=5.8)
txt(AXc + AWc / 2, YA - 0.08, "bits 31…0", c=B, fs=5.8)
txt(AXc + AWc / 2, YA - 0.24, r"1$\times$1600", fs=6.0)
txt(AXc + AWc / 2, YA - 0.37, r"2$\times$800", c=B, fs=6.0)
XQ = AXc + AWc + 0.10
bx = [5.64, 5.93, 6.22, 6.47, 6.72]
bw = [0.20, 0.20, 0.16, 0.16, 0.20]
bl = [r"$\theta$", r"$\rho$", r"$\pi$", r"$\chi$", r"$\iota$"]
dep = [True, True, False, False, True]
BH = 0.34
wire([(AXc + AWc, YA), (bx[0], YA)], lw=LB, arrow=True)
dot(XQ, YA, r=0.026)
for i in range(5):
    box(bx[i], YA - BH / 2, bw[i], BH, bl[i], fs=8.6, ec=B if dep[i] else K, lw=1.3 if dep[i] else 0.8)
    if i:
        wire([(bx[i - 1] + bw[i - 1], YA), (bx[i], YA)], lw=LB, arrow=True)
XE = 7.02; YFB = 2.64
wire([(bx[4] + bw[4], YA), (XE, YA), (XE, YFB), (4.44, YFB), (4.44, YA + 0.20), (MX, YA + 0.20)], lw=LB, arrow=True)
slash(5.95, YFB, "1600", dy=-0.085)
YM = YA + BH / 2 + 0.17
for i in (0, 1):
    cx = bx[i] + bw[i] / 2
    mux_d(cx, YM, 0.20, 0.10)
    wire([(cx, YM - 0.05), (cx, YA + BH / 2)], lw=0.8, c=B, arrow=True)
    for dx, t in ((-0.055, "64"), (0.055, "32")):
        wire([(cx + dx, YM + 0.05), (cx + dx, YM + 0.10)], lw=0.6, c=B)
        txt(cx + dx, YM + 0.155, t, c=B, fs=5.4)
    wire([(cx + 0.14, YD), (cx + 0.14, YM), (cx + 0.08, YM)], lw=0.7, c=B)
    dot(cx + 0.14, YD, c=B)
txt(bx[0] - 0.025, YM, r"$\times$10", c=B, fs=5.6, ha="right")
txt(bx[2] + 0.02, YM, r"$\times$712", c=B, fs=5.6, ha="left")
XDE = 6.965
wire([(4.30, YD), (XDE, YD)], lw=0.9, c=B)
wire([(4.07, YD), (4.07, YA + 0.13)], lw=0.7, c=B, arrow=True)
dot(4.07, YD, c=B)
ci = bx[4] + bw[4] / 2
YRM = YA - BH / 2 - 0.17
mux_u(ci, YRM, 0.20, 0.10)
wire([(ci, YRM + 0.05), (ci, YA - BH / 2)], lw=0.8, c=B, arrow=True)
box(ci - 0.14, 0.80, 0.28, 0.22, "RC", fs=6.4)
for dx in (-0.055, 0.055):
    wire([(ci + dx, 1.02), (ci + dx, YRM - 0.05)], lw=0.6, c=B)
txt(ci - 0.14, YRM, r"$\times$32", c=B, fs=5.6, ha="right")
wire([(XDE, YD), (XDE, YRM), (ci + 0.08, YRM)], lw=0.7, c=B)
reg(6.12, 0.80, 0.30, 0.22, "round", fs=6.2)
wire([(6.42, 0.91), (ci - 0.14, 0.91)], lw=LT, arrow=True)
txt(6.27, 0.71, "first…last", fs=5.6)
YT = 0.52
reg(AXc, YT - 0.14, AWc, 0.28, None, clk="br")
txt(AXc + AWc / 2 - 0.03, YT, r"$\mathbf{T}$ template", fs=6.2)
wire([(XQ, YA), (XQ, YT), (AXc + AWc, YT)], lw=LB, arrow=True)
txt(XQ + 0.045, 0.80, "digest", fs=5.6, ha="left")
wire([(AXc, YT), (4.44, YT), (4.44, YA - 0.20), (MX, YA - 0.20)], lw=LB, arrow=True)
txt(4.60, YT + 0.065, "reload", fs=5.6)
ax.add_patch(Circle((AXc + 0.17, YT - 0.28), 0.062, fc="white", ec=B, lw=1.0, zorder=5))
txt(AXc + 0.17, YT - 0.28, "+1", fs=5.4)
wire([(AXc + 0.17, YT - 0.218), (AXc + 0.17, YT - 0.14)], lw=LT, arrow=True)
txt(AXc + 0.26, YT - 0.28, "chain step", fs=5.6, ha="left")

# padded message is kept in T (template)
XB = 4.262
dot(XB, YA, r=0.024)
wire([(XB, YA), (XB, 0.86), (AXc + 0.42, 0.86), (AXc + 0.42, YT + 0.14)], lw=LB, arrow=True)
txt(4.86, 0.92, "message", fs=5.6)
# FSM selects the source of A
wire([(FX + 0.24, 1.13), (MX + 0.055, 1.13), (MX + 0.055, YA - 0.31 + 0.093)], lw=LT, arrow=True)
YS = 0.12
wire([(XQ, YT), (XQ, YS), (3.90, YS), (3.90, YO), (XC + 0.20, YO)], lw=LB, arrow=True)
dot(XQ, YT, r=0.026)
txt(6.00, YS + 0.07, "squeeze", fs=5.6)

fig.savefig("fig_arch4.pdf", bbox_inches="tight", pad_inches=0.01)
fig.savefig("fig_arch4.png", dpi=300, bbox_inches="tight", pad_inches=0.01)
print("ok")
