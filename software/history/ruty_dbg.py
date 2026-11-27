# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Small probe: a few single batches, status and clock count printed. usage: python3 ruty_dbg.py ruty.bit
import sys, time, hashlib, numpy as np
from pynq import Overlay, MMIO, allocate
ol = Overlay(sys.argv[1]); reg = MMIO(0xA0000000, 0x1000); print("RESULT id=%08x" % reg.read(0x38))
NW = 1 << 16; ibuf = allocate(shape=(NW,), dtype=np.uint64); obuf = allocate(shape=(NW,), dtype=np.uint64)
lo = min(ibuf.device_address, obuf.device_address) >> 12; hi = (max(ibuf.device_address, obuf.device_address) + 8 * NW + 4095) >> 12
reg.write(0x28, lo); reg.write(0x2C, hi)
for ln, nj in ((0, 1), (0, 9), (0, 9), (32, 9)):
    wpm = -(-ln // 8); msg = bytes((7 * k + ln) & 255 for k in range(ln)); raw = np.zeros((nj, wpm * 8), dtype=np.uint8); raw[:, :ln] = np.frombuffer(msg, dtype=np.uint8)
    ibuf[:nj * wpm] = raw.reshape(-1).view(np.uint64); ibuf.flush(); obuf[:nj * 4] = 0; obuf.flush()
    reg.write(0x08, ibuf.device_address & 0xffffffff); reg.write(0x0C, ibuf.device_address >> 32)
    reg.write(0x10, obuf.device_address & 0xffffffff); reg.write(0x14, obuf.device_address >> 32)
    reg.write(0x18, nj); reg.write(0x1C, ln | (0x1F << 24)); reg.write(0x20, 32); reg.write(0x24, (23 << 8) | (21 << 24))
    reg.write(0x00, 2); reg.write(0x00, 1); t0 = time.time()
    while reg.read(0x04) & 1 and time.time() - t0 < 2: pass
    st = reg.read(0x04); ck = reg.read(0x34); obuf.invalidate(); o = bytes(np.array(obuf[:nj * 4]).view(np.uint8)); exp = hashlib.shake_128(msg).digest(32)
    good = sum(o[32 * k:32 * k + 32] == exp for k in range(nj))
    print("RESULT len=%d jobs=%d status=%02x clocks=%d correct=%d/%d dbg0=%08x dbg1=%08x" % (ln, nj, st, ck, good, nj, reg.read(0x3C), reg.read(0x40)), flush=True)
    w = np.array(obuf[:nj * 4 + 8]); e = np.frombuffer(exp, dtype=np.uint64); print('RESULT   expect ' + ' '.join('%016x' % x for x in e)); [print('RESULT   words %3d: ' % k + ' '.join('%016x' % x for x in w[k:k+4])) for k in range(0, min(len(w), 44), 4)]
    if st & 1: ol = Overlay(sys.argv[1]); reg.write(0x28, lo); reg.write(0x2C, hi)
