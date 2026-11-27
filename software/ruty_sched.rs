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
unsafe fn to_device(p: *mut u8, n: usize) { let mut a = p as usize & !63; while a < p as usize + n { ruty_dc_cvac(a); a += 64; } ruty_dsb(); }
unsafe fn from_device(p: *mut u8, n: usize) { let mut a = p as usize & !63; while a < p as usize + n { ruty_dc_civac(a); a += 64; } ruty_dsb(); }
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
unsafe fn sched(engs: &[Eng], used: usize, g: &Alg, dom: u8, seed: &[u8], sks: &[u8]) -> (f64, Vec<Vec<u8>>, usize, bool) {
    let mut trees = new_trees(); let mut at: Vec<Option<(usize, usize, usize)>> = vec![None; used];
    let nb = g.w / 8; let dual = g.w == 32; let mut done = 0; let mut batches = 0; let mut ok = true; let mut spins = 0u64;
    let t0 = now_ns();
    while done < NTREE { spins += 1; if spins > 400_000_000 { ok = false; break; }
        for a in 0..used { let e = &engs[a];
            if let Some((ti, cnt, pc)) = at[a] {
                if let Some(s) = e.finished() { ok &= s & 0x72 == 2; let ob = e.buf(1).0; from_device(ob, (if dual { pc / 2 } else { pc }) * 16 * if dual { 2 } else { 1 });
                    let t = &mut trees[ti]; let dst = match t.stage { 0 => t.sk.as_mut_ptr(), 1 => t.ends.as_mut_ptr(), _ => t.lvl.as_mut_ptr() };
                    for k in 0..cnt { rdg(ob, dual, k, dst.add(k * 16)); }
                    if t.stage >= 2 { t.nl = cnt; } t.stage += 1; t.busy = false; if t.stage == 6 { done += 1; } at[a] = None; } }
            if at[a].is_none() {
                let mut pick: Option<usize> = None;
                for (i, t) in trees.iter().enumerate() { if !t.busy && t.stage < 6 && pick.map_or(true, |p| t.stage > trees[p].stage) { pick = Some(i); } }
                if let Some(ti) = pick { let (cnt, pc, ln, chain) = fill(e, &trees[ti], dual, seed, sks);
                    e.wr(LEN, ln as u32 | ((dom as u32) << 24)); e.wr(OLEN, 16); e.wr(ALG, g.first | (g.last << 8) | ((dual as u32) << 16) | ((g.rate as u32 / nb) << 24));
                    if chain { e.wr(0x44, 1 | ((47 / nb) << 8) | ((47 % nb) << 16) | ((48 / nb) << 20)); e.wr(0x48, 15); } else { e.wr(0x44, 0); }
                    e.start(e.buf(0).1, e.buf(1).1, if dual { pc / 2 } else { pc }); trees[ti].busy = true; at[a] = Some((ti, cnt, pc)); batches += 1; } } } }
    let dt = now_ns() - t0; for e in engs.iter() { e.wr(0x44, 0); }
    (dt, trees.iter().map(|t| if t.stage == 6 { t.lvl[0..16].to_vec() } else { vec![] }).collect(), batches, ok) }
fn main() { unsafe {
    let args: Vec<String> = std::env::args().collect(); let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let emhz: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let fd = open(b"/dev/mem\0".as_ptr() as *const c_char, 2 | 0x101000); if fd < 0 { println!("RESULT error=open_dev_mem"); return; }
    let mut pm = File::open("/proc/self/pagemap").unwrap(); let mut engs = Vec::new();
    let hf = open(b"/dev/dma_heap/reserved\0".as_ptr() as *const c_char, 2); if hf < 0 { println!("RESULT error=open_dma_heap"); return; }
    for i in 0..n {
        let reg = mmap(std::ptr::null_mut(), 4096, 3, 1, fd, 0xA000_0000i64 + 0x1000 * i as i64) as *mut u32;
        let mut a = HeapAlloc { len: PAGE as u64, fd: 0, fd_flags: 2 | 0x80000, heap_flags: 0 };
        if ioctl(hf, 0xC018_4800, &mut a as *mut HeapAlloc) != 0 { println!("RESULT error=no_contiguous_buffer"); return; }
        let va = mmap(std::ptr::null_mut(), PAGE, 3, 1, a.fd as c_int, 0) as *mut u8;
        if va as isize == -1 { println!("RESULT error=cannot_map_buffer"); return; }
        let mut k = 0; while k < PAGE { *va.add(k) = 0; k += 4096; }
        let mut pfn = [0u64; 2];
        for (j, off) in [0usize, PAGE - 4096].iter().enumerate() {
            let mut e = [0u8; 8]; pm.seek(SeekFrom::Start(((va as u64 + *off as u64) / 4096) * 8)).unwrap(); pm.read_exact(&mut e).unwrap();
            let ent = u64::from_le_bytes(e); if ent >> 63 == 0 || ent & ((1 << 55) - 1) == 0 { println!("RESULT error=no_physical_address (run as root)"); return; }
            pfn[j] = ent & ((1 << 55) - 1); }
        let pa = pfn[0] * 4096;
        if pfn[1] - pfn[0] != (PAGE as u64 / 4096) - 1 || pa + (PAGE as u64) > LIMIT { println!("RESULT error=buffer_not_usable pa={:#x}", pa); return; }
        let g = Eng { reg, va, pa }; g.wr(WBASE, (pa >> 12) as u32); g.wr(WLIMIT, ((pa + PAGE as u64) >> 12) as u32); engs.push(g); }
    println!("RESULT sched engines={} engine_mhz={} id={:08x} job=22_trees_of_SLH-DSA-128f hashes_per_job={}", n, emhz, engs[0].rd(ID), NTREE * (NCH * 16 + NLEAF + NLEAF - 1));
    let seed: Vec<u8> = (0..16u8).collect(); let sks: Vec<u8> = (100..116u8).collect(); let mut bad = 0;
    for &(gi, dom) in [(1usize, 0x1Fu8), (3, 0x1F), (4, 0x01)].iter() { let g = &ALGS[gi];
        let want: Vec<Vec<u8>> = new_trees().iter().map(|t| ref_root(g, dom, &seed, &sks, t.layer, t.tree)).collect();
        let hx = |v: &[u8]| v.iter().map(|b| format!("{:02x}", b)).collect::<String>(); println!("RESULT sched root alg={} tree0={} tree21={}", g.name, hx(&want[0]), hx(&want[21]));
        for &used in [1usize, 2, 4, 8].iter() { if used > n { continue; }
            let mut best = f64::MAX; let mut nbat = 0; let mut miss = 0;
            for _ in 0..5 { let (dt, roots, b, ok) = sched(&engs, used, g, dom, &seed, &sks); nbat = b;
                let m = roots.iter().zip(want.iter()).filter(|(a, b)| a != b).count() + (!ok) as usize; miss += m; if m == 0 && dt < best { best = dt; } }
            bad += miss;
            println!("RESULT sched2 alg={} atoms={} ms={:.3} batches={} roots_wrong={}", g.name, used, best / 1e6, nbat, miss); } }
    println!("RESULT sched2 summary roots_wrong={} {}", bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
