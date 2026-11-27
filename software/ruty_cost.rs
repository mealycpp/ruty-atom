// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty: stand-alone Rust driver and test for the RUTY bus-master image (no Python, no external crates).
//   gcc -c cacheops.S -o cacheops.o && rustc -O -C link-arg=cacheops.o -o ruty ruty.rs
//   sudo ./ruty <engines> [bus MHz]          (the image must already be loaded; huge pages must be available)
// Registers: /dev/mem. Buffers: one 2 MB huge page per engine (physical address from /proc/self/pagemap), split into two
// halves A and B; the engine's memory window is set to exactly that page.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::ptr::{read_volatile, write_volatile};
#[repr(C)] struct Timespec { tv_sec: c_long, tv_nsec: c_long }
extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) -> *mut c_void;
    fn mlock(addr: *const c_void, len: usize) -> c_int;
    fn clock_gettime(clk: c_int, ts: *mut Timespec) -> c_int;
}
#[cfg(target_arch = "aarch64")] extern "C" { fn ruty_dc_cvac(a: usize); fn ruty_dc_civac(a: usize); fn ruty_dsb(); }
#[cfg(not(target_arch = "aarch64"))] unsafe fn ruty_dc_cvac(_a: usize) {}
#[cfg(not(target_arch = "aarch64"))] unsafe fn ruty_dc_civac(_a: usize) {}
#[cfg(not(target_arch = "aarch64"))] unsafe fn ruty_dsb() {}
const CTRL: usize = 0x00; const STATUS: usize = 0x04; const IN_LO: usize = 0x08; const IN_HI: usize = 0x0C; const OUT_LO: usize = 0x10;
const OUT_HI: usize = 0x14; const NJOBS: usize = 0x18; const LEN: usize = 0x1C; const OLEN: usize = 0x20; const ALG: usize = 0x24;
const WBASE: usize = 0x28; const WLIMIT: usize = 0x2C; const CLOCKS: usize = 0x34; const ID: usize = 0x38;
const PAGE: usize = 2 << 20; const HALF: usize = PAGE / 2;
fn now_ns() -> f64 { let mut t = Timespec { tv_sec: 0, tv_nsec: 0 }; unsafe { clock_gettime(1, &mut t); } t.tv_sec as f64 * 1e9 + t.tv_nsec as f64 }

// ---------------- software reference: Keccak-p with 64-bit or 32-bit lanes ----------------
const RHO: [u32; 25] = [0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14];
fn rc(r: u32) -> u64 { let mut l: u32 = 1; let mut v = 0u64;                      // round constant r, 64 bits
    for _ in 0..r { for _ in 0..7 { l = (l << 1) ^ (if l & 0x80 != 0 { 0x171 } else { 0 }); } }
    for j in 0..7 { if l & 1 != 0 { v |= 1u64 << ((1u32 << j) - 1); } l = (l << 1) ^ (if l & 0x80 != 0 { 0x171 } else { 0 }); } v }
fn rol(x: u64, n: u32, w: u32) -> u64 { let n = n % w; let m = if w == 64 { !0u64 } else { 0xffff_ffff }; if n == 0 { x } else { ((x << n) | (x >> (w - n))) & m } }
fn keccak_p(a: &mut [u64; 25], w: u32, first: u32, last: u32) { let m = if w == 64 { !0u64 } else { 0xffff_ffff };
    for r in first..=last {
        let mut c = [0u64; 5]; for x in 0..5 { c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20]; }
        for i in 0..25 { a[i] ^= c[(i % 5 + 4) % 5] ^ rol(c[(i % 5 + 1) % 5], 1, w); }
        let mut b = [0u64; 25]; for x in 0..5 { for y in 0..5 { b[y + 5 * ((2 * x + 3 * y) % 5)] = rol(a[x + 5 * y], RHO[x + 5 * y], w); } }
        for i in 0..25 { a[i] = b[i] ^ (!b[(i % 5 + 1) % 5 + 5 * (i / 5)] & m & b[(i % 5 + 2) % 5 + 5 * (i / 5)]); }
        a[0] ^= rc(r) & m; } }
#[derive(Clone, Copy)] struct Alg { name: &'static str, w: u32, rate: usize, first: u32, last: u32 }
const ALGS: [Alg; 6] = [Alg { name: "shake128", w: 64, rate: 168, first: 0, last: 23 }, Alg { name: "shake256", w: 64, rate: 136, first: 0, last: 23 },
    Alg { name: "turboshake128", w: 64, rate: 168, first: 12, last: 23 }, Alg { name: "turboshake256", w: 64, rate: 136, first: 12, last: 23 },
    Alg { name: "minishake128", w: 32, rate: 68, first: 10, last: 21 }, Alg { name: "minishake256", w: 32, rate: 36, first: 10, last: 21 }];
fn xof(g: &Alg, msg: &[u8], outlen: usize) -> Vec<u8> { let nb = (g.w / 8) as usize; let mut st = [0u8; 200]; let mut pos = 0; let mut out = Vec::new();
    let perm = |st: &mut [u8; 200]| { let mut a = [0u64; 25];
        for i in 0..25 { let mut v = 0u64; for k in 0..nb { v |= (st[nb * i + k] as u64) << (8 * k); } a[i] = v; }
        keccak_p(&mut a, g.w, g.first, g.last);
        for i in 0..25 { for k in 0..nb { st[nb * i + k] = (a[i] >> (8 * k)) as u8; } } };
    for &b in msg { st[pos] ^= b; pos += 1; if pos == g.rate { perm(&mut st); pos = 0; } }
    st[pos] ^= 0x1F; st[g.rate - 1] ^= 0x80;
    while out.len() < outlen { perm(&mut st); out.extend_from_slice(&st[..g.rate]); }
    out.truncate(outlen); out }

// ---------------- one engine ----------------
struct Eng { reg: *mut u32, va: *mut u8, pa: u64 }
impl Eng {
    unsafe fn wr(&self, off: usize, v: u32) { write_volatile(self.reg.add(off / 4), v) }
    unsafe fn rd(&self, off: usize) -> u32 { read_volatile(self.reg.add(off / 4)) }
    fn buf(&self, half: usize) -> (*mut u8, u64) { unsafe { (self.va.add(half * HALF), self.pa + (half * HALF) as u64) } }
    unsafe fn shape(&self, g: &Alg, len: usize, olen: usize) { let nb = g.w / 8; let dual = (g.w == 32) as u32;
        self.wr(LEN, len as u32 | (0x1F << 24)); self.wr(OLEN, olen as u32); self.wr(ALG, g.first | (g.last << 8) | (dual << 16) | ((g.rate as u32 / nb) << 24)); }
    unsafe fn start(&self, inp: u64, out: u64, njobs: usize) { self.wr(IN_LO, inp as u32); self.wr(IN_HI, (inp >> 32) as u32); self.wr(OUT_LO, out as u32);
        self.wr(OUT_HI, (out >> 32) as u32); self.wr(NJOBS, njobs as u32); self.wr(CTRL, 2); self.wr(CTRL, 1); }
    unsafe fn finished(&self) -> Option<u32> { let s = self.rd(STATUS); if s & 1 == 0 && s & 0x42 != 0 { Some(s) } else { None } }
    unsafe fn wait(&self) -> u32 { let mut n = 0u32; loop { if let Some(s) = self.finished() { return s; } n += 1; if n > 80_000_000 { return 0xFFFF_FFFF; } } }
    unsafe fn run(&self, inp: u64, out: u64, njobs: usize) -> bool { self.start(inp, out, njobs); self.wait() & 0x72 == 2 }
}
static mut FLUSH: bool = true;
unsafe fn to_device(p: *mut u8, n: usize) { if !FLUSH { return; } let mut a = p as usize & !63; while a < p as usize + n { ruty_dc_cvac(a); a += 64; } ruty_dsb(); }
unsafe fn from_device(p: *mut u8, n: usize) { if !FLUSH { return; } let mut a = p as usize & !63; while a < p as usize + n { ruty_dc_civac(a); a += 64; } ruty_dsb(); }
/// equal-length messages -> bytes in the layout RUTY reads (one 64-bit word = one lane; two cores: 4 bytes of each job per word)
fn pack(g: &Alg, msgs: &[Vec<u8>]) -> Vec<u8> { let ln = msgs[0].len();
    if g.w == 64 { let wpm = (ln + 7) / 8; let mut o = vec![0u8; msgs.len() * wpm * 8]; for (k, m) in msgs.iter().enumerate() { o[k * wpm * 8..k * wpm * 8 + ln].copy_from_slice(m); } o }
    else { let wpm = (ln + 3) / 4; let mut o = vec![0u8; msgs.len() / 2 * wpm * 8];
        for (k, m) in msgs.iter().enumerate() { let (j, h) = (k / 2, k % 2); for (i, &b) in m.iter().enumerate() { o[j * wpm * 8 + (i / 4) * 8 + h * 4 + i % 4] = b; } } o } }
fn unpack(g: &Alg, raw: &[u8], n: usize, olen: usize) -> Vec<Vec<u8>> {
    if g.w == 64 { let wpo = (olen + 7) / 8; (0..n).map(|k| raw[k * wpo * 8..k * wpo * 8 + olen].to_vec()).collect() }
    else { let wpo = (olen + 3) / 4; (0..n).map(|k| { let (j, h) = (k / 2, k % 2); (0..olen).map(|i| raw[j * wpo * 8 + (i / 4) * 8 + h * 4 + i % 4]).collect() }).collect() } }
unsafe fn put(e: &Eng, half: usize, data: &[u8]) { let (p, _) = e.buf(half); std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len()); to_device(p, data.len()); }
unsafe fn get(e: &Eng, half: usize, n: usize) -> Vec<u8> { let (p, _) = e.buf(half); from_device(p, n); std::slice::from_raw_parts(p, n).to_vec() }
fn jobs(g: &Alg, n: usize) -> usize { if g.w == 32 { n / 2 } else { n } }
fn owords(g: &Alg, olen: usize) -> usize { if g.w == 32 { (olen + 3) / 4 } else { (olen + 7) / 8 } }
struct Rng(u64); impl Rng { fn byte(&mut self) -> u8 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 24) as u8 }
    fn msg(&mut self, n: usize) -> Vec<u8> { (0..n).map(|_| self.byte()).collect() } }

#[repr(C)] struct HeapAlloc { len: u64, fd: u32, fd_flags: u32, heap_flags: u64 }
extern "C" { fn ioctl(fd: c_int, req: u64, arg: *mut HeapAlloc) -> c_int; }
const LIMIT: u64 = 0x8000_0000;                          // the image maps the low 2 GB of memory to the engines

// ================= ruty_sched: a software scheduler that makes N atoms work together on one job =================
// Job: the 22 XMSS trees of one SLH-DSA-128f signature (FIPS 205 address formats, n = 16): per tree 280 secrets (PRF), 280 chains of
// 15 steps run inside an atom, 8 key compressions (608 bytes) and 7 tree nodes. No atom has a fixed role: whenever an atom is free it
// takes the most advanced stage that is ready, of any tree, and is set to the function and operation that stage needs.
// Messages are written straight into the atom's buffer and digests read straight out of it (no allocation in the loop).
//   sudo ./ruty_sched <engines in the image> [engine MHz]
const NLEAF: usize = 8; const LEN_W: usize = 35; const NCH: usize = NLEAF * LEN_W; const NTREE: usize = 22;
fn adrs(layer: u32, tree: u64, typ: u32, kp: u32, a: u32, b: u32) -> [u8; 32] { let mut o = [0u8; 32];
    o[0..4].copy_from_slice(&layer.to_be_bytes()); o[8..16].copy_from_slice(&tree.to_be_bytes()); o[16..20].copy_from_slice(&typ.to_be_bytes());
    o[20..24].copy_from_slice(&kp.to_be_bytes()); o[24..28].copy_from_slice(&a.to_be_bytes()); o[28..32].copy_from_slice(&b.to_be_bytes()); o }
fn xofd(g: &Alg, dom: u8, msg: &[u8], outlen: usize) -> Vec<u8> { let nb = (g.w / 8) as usize; let mut st = [0u8; 200]; let mut pos = 0; let mut out = Vec::new();
    let perm = |st: &mut [u8; 200]| { let mut a = [0u64; 25];
        for i in 0..25 { let mut v = 0u64; for k in 0..nb { v |= (st[nb * i + k] as u64) << (8 * k); } a[i] = v; }
        keccak_p(&mut a, g.w, g.first, g.last);
        for i in 0..25 { for k in 0..nb { st[nb * i + k] = (a[i] >> (8 * k)) as u8; } } };
    for &b in msg { st[pos] ^= b; pos += 1; if pos == g.rate { perm(&mut st); pos = 0; } }
    st[pos] ^= dom; st[g.rate - 1] ^= 0x80;
    while out.len() < outlen { perm(&mut st); out.extend_from_slice(&st[..g.rate]); }
    out.truncate(outlen); out }
fn cat(parts: &[&[u8]]) -> Vec<u8> { let mut v = Vec::with_capacity(96); for p in parts { v.extend_from_slice(p); } v }
fn ref_root(g: &Alg, dom: u8, seed: &[u8], sks: &[u8], layer: u32, tree: u64) -> Vec<u8> {
    let mut lvl: Vec<Vec<u8>> = (0..NLEAF).map(|l| { let mut m = cat(&[seed, &adrs(layer, tree, 1, l as u32, 0, 0)]);
        for c in 0..LEN_W { let mut x = xofd(g, dom, &cat(&[seed, &adrs(layer, tree, 5, l as u32, c as u32, 0), sks]), 16);
            for s in 0..15u32 { x = xofd(g, dom, &cat(&[seed, &adrs(layer, tree, 0, l as u32, c as u32, s), &x]), 16); } m.extend_from_slice(&x); }
        xofd(g, dom, &m, 16) }).collect();
    let mut z = 1u32; while lvl.len() > 1 { lvl = (0..lvl.len() / 2).map(|i| xofd(g, dom, &cat(&[seed, &adrs(layer, tree, 2, 0, z, i as u32), &lvl[2 * i], &lvl[2 * i + 1]]), 16)).collect(); z += 1; }
    lvl.remove(0) }
// stages: 0 secrets, 1 chains (in the atom), 2 key compression, 3..5 tree levels, 6 done
struct Tree { layer: u32, tree: u64, stage: usize, busy: bool, sk: Vec<u8>, ends: Vec<u8>, lvl: Vec<u8>, nl: usize }
fn new_trees() -> Vec<Tree> { (0..NTREE).map(|t| Tree { layer: t as u32, tree: 0x11_2233_4455 + t as u64, stage: 0, busy: false, sk: vec![0; NCH * 16], ends: vec![0; NCH * 16], lvl: vec![0; NLEAF * 16], nl: NLEAF }).collect() }
#[inline(always)] unsafe fn wp(base: *mut u8, dual: bool, wpm: usize, k: usize, off: usize, src: &[u8]) {
    if !dual { std::ptr::copy_nonoverlapping(src.as_ptr(), base.add(k * wpm * 8 + off), src.len()); }
    else { let b = base.add((k / 2) * wpm * 8 + (k % 2) * 4); let mut c = 0; while c < src.len() { std::ptr::copy_nonoverlapping(src.as_ptr().add(c), b.add(((off + c) / 4) * 8), 4); c += 4; } } }
#[inline(always)] unsafe fn rdg(base: *const u8, dual: bool, k: usize, dst: *mut u8) {
    if !dual { std::ptr::copy_nonoverlapping(base.add(k * 16), dst, 16); }
    else { let b = base.add((k / 2) * 32 + (k % 2) * 4); for c in 0..4 { std::ptr::copy_nonoverlapping(b.add(c * 8), dst.add(c * 4), 4); } } }
unsafe fn fill(e: &Eng, t: &Tree, dual: bool, seed: &[u8], sks: &[u8]) -> (usize, usize, usize, bool) {     // returns jobs, padded jobs, length, chain
    let base = e.buf(0).0; let (cnt, ln, chain) = match t.stage { 0 => (NCH, 64, false), 1 => (NCH, 64, true), 2 => (NLEAF, 48 + 16 * LEN_W, false), _ => (t.nl / 2, 80, false) };
    let pc = if dual && cnt % 2 == 1 { cnt + 1 } else { cnt }; let wpm = if dual { (ln + 3) / 4 } else { (ln + 7) / 8 };
    for k in 0..pc { let c = k % cnt; wp(base, dual, wpm, k, 0, seed);
        match t.stage {
            0 => { wp(base, dual, wpm, k, 16, &adrs(t.layer, t.tree, 5, (c / LEN_W) as u32, (c % LEN_W) as u32, 0)); wp(base, dual, wpm, k, 48, sks); }
            1 => { wp(base, dual, wpm, k, 16, &adrs(t.layer, t.tree, 0, (c / LEN_W) as u32, (c % LEN_W) as u32, 0)); wp(base, dual, wpm, k, 48, &t.sk[c * 16..c * 16 + 16]); }
            2 => { wp(base, dual, wpm, k, 16, &adrs(t.layer, t.tree, 1, c as u32, 0, 0)); wp(base, dual, wpm, k, 48, &t.ends[c * LEN_W * 16..(c + 1) * LEN_W * 16]); }
            _ => { wp(base, dual, wpm, k, 16, &adrs(t.layer, t.tree, 2, 0, (t.stage - 2) as u32, c as u32)); wp(base, dual, wpm, k, 48, &t.lvl[c * 32..c * 32 + 32]); } } }
    to_device(base, (if dual { pc / 2 } else { pc }) * wpm * 8); (cnt, pc, ln, chain) }

// ================= ruty_cost: the processor-side routines of the job-graph scheduler, timed one by one =================
// The same binary runs on the board (real Cortex-A53) and in gem5 (model of it). No device is touched: registers are plain memory,
// buffers are ordinary pages. What is timed is what the processor does for a node: lay out its data, flush, collect its digests.
//   ./ruty_cost <flush 1|0> <repetitions>
fn tm<F: FnMut(usize)>(name: &str, reps: usize, mut f: F) { f(0); let mut mn = f64::MAX; let mut sum = 0.0;
    for r in 0..reps { let t0 = now_ns(); f(r + 1); let d = now_ns() - t0; if d < mn { mn = d; } sum += d; }
    println!("COST {} min_ns={:.0} mean_ns={:.0} reps={}", name, mn, sum / reps as f64, reps); }
fn main() { unsafe {
    let args: Vec<String> = std::env::args().collect(); FLUSH = args.get(1).map_or(true, |s| s != "0");
    let reps: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
    let mut regs = vec![0u32; 1024]; let lay = std::alloc::Layout::from_size_align(PAGE, 4096).unwrap(); let va = std::alloc::alloc_zeroed(lay);
    let e = Eng { reg: regs.as_mut_ptr(), va, pa: 0 };
    let seed: Vec<u8> = (0..16u8).collect(); let sks: Vec<u8> = (100..116u8).collect(); let mut trees = new_trees();
    let mut rng = Rng(0x5EED_0DF6_2026_1126); let mut tags = rng.msg(16 * 240000); let mut fw = rng.msg(4 << 20); let mut sink = vec![0u8; 38400];
    println!("COST start flush={} reps={}", FLUSH, reps);
    tm("fill_secrets_280", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 0; fill(&e, t, false, &seed, &sks); });
    tm("fill_chains_280", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 1; fill(&e, t, false, &seed, &sks); });
    tm("fill_compress_8", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 2; fill(&e, t, false, &seed, &sks); });
    tm("fill_level_4", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 3; t.nl = 8; fill(&e, t, false, &seed, &sks); });
    tm("collect_280", reps, |r| { let t = &mut trees[r % NTREE]; let ob = e.buf(1).0; from_device(ob, 280 * 16); let dst = t.ends.as_mut_ptr(); for k in 0..280 { rdg(ob, false, k, dst.add(k * 16)); } });
    tm("collect_8", reps, |r| { let t = &mut trees[r % NTREE]; let ob = e.buf(1).0; from_device(ob, 8 * 16); let dst = t.lvl.as_mut_ptr(); for k in 0..8 { rdg(ob, false, k, dst.add(k * 16)); } });
    tm("tags_copy_240kB", reps, |r| { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(tags.as_ptr().add((r % 16) * 240000), p, 240000); to_device(p, 240000); });
    tm("tags_collect_38kB", reps, |_r| { let ob = e.buf(1).0; from_device(ob, 38400); std::ptr::copy_nonoverlapping(ob, sink.as_mut_ptr(), 38400); });
    tm("firmware_copy_1MB", reps, |r| { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(fw.as_ptr().add((r % 4) << 20), p, 1 << 20); to_device(p, 1 << 20); });
    tm("tags_flush_only_240kB", reps, |r| { let p = tags.as_mut_ptr().add((r % 16) * 240000); *p = (*p).wrapping_add(1); to_device(p, 240000); });
    tm("firmware_flush_only_1MB", reps, |r| { let p = fw.as_mut_ptr().add((r % 4) << 20); *p = (*p).wrapping_add(1); to_device(p, 1 << 20); });
    let mut acc = 0u32;
    tm("poll_1000_reads", reps, |_r| { for _ in 0..1000 { acc = acc.wrapping_add(e.rd(STATUS)); } });
    tm("shape_5_writes", reps, |r| { e.wr(LEN, r as u32); e.wr(OLEN, 16); e.wr(ALG, 1); e.wr(0x44, 0); e.wr(0x48, 15); });
    println!("COST end acc={} sink={}", acc, sink[0]);
} }
