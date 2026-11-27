#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""README banner: RSCL seal + title + the split motif (one 5x5 lane register drawn as one 1600-bit state and as two 800-bit states)."""
from PIL import Image, ImageDraw, ImageFont
G = "/usr/share/fonts/truetype/google-fonts/ofl/"
W, H = 2400, 660; BG = (6, 18, 12); GOLD = (214, 170, 78); PALE = (236, 226, 200); DIM = (140, 150, 140); BLUE = (86, 140, 220)
im = Image.new("RGB", (W, H), BG); d = ImageDraw.Draw(im)
# faint circuit grid
for x in range(0, W, 60): d.line([(x, 0), (x, H)], fill=(10, 26, 18), width=1)
for y in range(0, H, 60): d.line([(0, y), (W, y)], fill=(10, 26, 18), width=1)
d.rectangle([0, 0, W - 1, 7], fill=GOLD); d.rectangle([0, H - 8, W - 1, H - 1], fill=GOLD)
seal = Image.open("../release_assets/rscl_seal.png").convert("RGBA").resize((500, 500), Image.LANCZOS); im.paste(seal, (70, 80), seal)
f_k = ImageFont.truetype(G + "firasans/FiraSans-Medium.ttf", 34); f_t = ImageFont.truetype(G + "cardo/Cardo-Bold.ttf", 150)
f_s = ImageFont.truetype(G + "cardo/Cardo-Regular.ttf", 50); f_m = ImageFont.truetype(G + "firasans/FiraSans-Light.ttf", 33); f_l = ImageFont.truetype(G + "firasans/FiraSans-Medium.ttf", 26)
x0 = 640
d.text((x0, 96), "RECONFIGURABLE SPACE COMPUTING LAB  ·  CAL POLY POMONA", font=f_k, fill=GOLD)
d.line([(x0, 150), (x0 + 150, 150)], fill=GOLD, width=5)
d.text((x0 - 6, 160), "RUTY-ATOM", font=f_t, fill=PALE)
d.text((x0, 345), "A run-time splittable Keccak core", font=f_s, fill=PALE)
d.text((x0, 410), "for embedded roots of trust", font=f_s, fill=PALE)
d.text((x0, 505), "One 1600-bit state register. One register write. One Keccak-p[1600] or two Keccak-p[800].", font=f_m, fill=DIM)
d.text((x0, 550), "SHAKE · TurboSHAKE · MiniSHAKE  —  one round per clock in every setting", font=f_m, fill=DIM)
# split motif
def grid(x, y, cell, split):
    for r in range(5):
        for c in range(5):
            X, Y = x + c * (cell + 6), y + r * (cell + 6)
            if split:
                d.rectangle([X, Y, X + cell // 2 - 2, Y + cell], fill=GOLD); d.rectangle([X + cell // 2 + 2, Y, X + cell, Y + cell], fill=BLUE)
            else:
                d.rectangle([X, Y, X + cell, Y + cell], fill=GOLD)
gx, gy, cell = 1830, 95, 38
grid(gx, gy, cell, False); grid(gx + 290, gy, cell, True)
d.text((gx + 4, gy + 232), "1 × 1600", font=f_l, fill=PALE); d.text((gx + 300, gy + 232), "2 × 800", font=f_l, fill=PALE)
d.text((gx + 233, gy + 82), "⇄", font=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 44), fill=PALE)
d.text((gx + 225, gy + 140), "dual", font=f_l, fill=DIM)
im.save("docs/img/banner.png", optimize=True); print(im.size)
