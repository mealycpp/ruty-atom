/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Copyright (c) 2026 Mohamed El-Hadedy */
// Cache maintenance for memory shared with the fabric (allowed in user mode on arm64 Linux).
        .text
        .global ruty_dc_cvac
ruty_dc_cvac:   dc cvac, x0             // write the cache line back to memory
                ret
        .global ruty_dc_civac
ruty_dc_civac:  dc civac, x0            // write back and drop the cache line
                ret
        .global ruty_dsb
ruty_dsb:       dsb sy
                ret
