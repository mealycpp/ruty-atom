// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_drv: RUTY batch driver (Rust, no external crates). Built as a C-callable library:
//   rustc -O --crate-type cdylib -o libruty_drv.so ruty_drv.rs
// The caller supplies the physical addresses of two DMA-able buffers. All register access goes through /dev/mem.
// Kernels run as back-to-back batches in which the digest buffer of one batch is the message buffer of the next,
// so the processor never touches the data between batches.
#![allow(non_camel_case_types)]
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::ptr::{read_volatile, write_volatile};
#[repr(C)] struct timespec { tv_sec: c_long, tv_nsec: c_long }
extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) -> *mut c_void;
    fn clock_gettime(clk: c_int, ts: *mut timespec) -> c_int;
}
const CTRL: usize = 0x00; const STATUS: usize = 0x04; const IN_LO: usize = 0x08; const IN_HI: usize = 0x0C;
const OUT_LO: usize = 0x10; const OUT_HI: usize = 0x14; const NJOBS: usize = 0x18; const LEN: usize = 0x1C;
const OLEN: usize = 0x20; const ALG: usize = 0x24; const CLOCKS: usize = 0x34;
static mut REG: *mut u32 = std::ptr::null_mut();
unsafe fn wr(off: usize, v: u32) { write_volatile(REG.add(off / 4), v) }
unsafe fn rd(off: usize) -> u32 { read_volatile(REG.add(off / 4)) }
fn now_ns() -> f64 { let mut t = timespec { tv_sec: 0, tv_nsec: 0 }; unsafe { clock_gettime(1, &mut t); } t.tv_sec as f64 * 1e9 + t.tv_nsec as f64 }

/// Map the register page. Returns 0 on success.
#[no_mangle] pub unsafe extern "C" fn ruty_open(base: u64) -> c_int {
    let fd = open(b"/dev/mem\0".as_ptr() as *const c_char, 2 | 0x101000);
    if fd < 0 { return -1; }
    let p = mmap(std::ptr::null_mut(), 4096, 3, 1, fd, base as i64);
    if p as isize == -1 { return -2; }
    REG = p as *mut u32; 0
}
/// One batch. Returns the status register (bit 1 done, bit 6 window refusal, bits 5:4 engine refusals), or 0xFFFF_FFFF on time-out.
unsafe fn batch(inp: u64, out: u64, njobs: u32) -> u32 {
    wr(IN_LO, inp as u32); wr(IN_HI, (inp >> 32) as u32); wr(OUT_LO, out as u32); wr(OUT_HI, (out >> 32) as u32);
    wr(NJOBS, njobs); wr(CTRL, 2); wr(CTRL, 1);
    let mut n = 0u32;
    loop { let s = rd(STATUS); if s & 1 == 0 && s & 0x42 != 0 { return s; } n += 1; if n > 50_000_000 { return 0xFFFF_FFFF; } }
}
unsafe fn shape(len: u32, dom: u32, olen: u32, alg: u32) { wr(LEN, len | (dom << 24)); wr(OLEN, olen); wr(ALG, alg); }

/// Hash chains: `steps` batches of `njobs` jobs, len bytes in and len bytes out, ping-pong between the two buffers.
/// Returns elapsed ns (negative = error code); *clocks receives the bus clocks counted in hardware.
#[no_mangle] pub unsafe extern "C" fn ruty_chains(a: u64, b: u64, njobs: u32, len: u32, alg: u32, steps: u32, clocks: *mut u64) -> f64 {
    let (mut x, mut y, mut ck) = (a, b, 0u64);
    let t0 = now_ns();
    shape(len, 0x1F, len, alg);
    for _ in 0..steps { let s = batch(x, y, njobs); if s & 0x72 != 2 { return -(s as f64) - 1.0; } ck += rd(CLOCKS) as u64; std::mem::swap(&mut x, &mut y); }
    let t1 = now_ns(); *clocks = ck; t1 - t0
}
/// Merkle tree: one batch of `leaves` jobs (32 bytes in, 32 out), then batches of leaves/2, leaves/4, ... 1 jobs (64 in, 32 out).
/// In two-core mode `leaves` counts pairs, i.e. the two cores build two half-trees side by side.
#[no_mangle] pub unsafe extern "C" fn ruty_tree(a: u64, b: u64, leaves: u32, alg: u32, reps: u32, clocks: *mut u64) -> f64 {
    let mut ck = 0u64;
    let t0 = now_ns();
    for _ in 0..reps {
        let (mut x, mut y) = (a, b);
        shape(32, 0x1F, 32, alg);
        let s = batch(x, y, leaves); if s & 0x72 != 2 { return -(s as f64) - 1.0; } ck += rd(CLOCKS) as u64; std::mem::swap(&mut x, &mut y);
        shape(64, 0x1F, 32, alg);
        let mut n = leaves / 2;
        while n >= 1 { let s = batch(x, y, n); if s & 0x72 != 2 { return -(s as f64) - 1.0; } ck += rd(CLOCKS) as u64; std::mem::swap(&mut x, &mut y); n /= 2; }
    }
    let t1 = now_ns(); *clocks = ck; t1 - t0
}
/// Plain batch of equal jobs, for the per-hash table. Returns elapsed ns.
#[no_mangle] pub unsafe extern "C" fn ruty_batch(inp: u64, out: u64, njobs: u32, len: u32, olen: u32, alg: u32, clocks: *mut u64) -> f64 {
    let t0 = now_ns(); shape(len, 0x1F, olen, alg);
    let s = batch(inp, out, njobs); let t1 = now_ns();
    if s & 0x72 != 2 { return -(s as f64) - 1.0; }
    *clocks = rd(CLOCKS) as u64; t1 - t0
}
