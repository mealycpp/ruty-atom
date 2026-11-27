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

// ================= ruty_apps: the application rows on the atoms =================
// Each application is a batch of equal-shaped jobs. The batch is split evenly over the atoms, all atoms start together.
// Two times are taken: run (first start to last finish) and e2e (copy the laid-out messages into the atom buffers, flush, run, read back).
// The hardware counter of every atom (bus clocks, go to last digest in memory) is read too; the largest one is reported.
// Every digest of every run is compared with the software reference. Best of 5 runs.
//   sudo ./ruty_apps <engines in the image> [engine MHz]
fn xofd(g: &Alg, dom: u8, msg: &[u8], outlen: usize) -> Vec<u8> { let nb = (g.w / 8) as usize; let mut st = [0u8; 200]; let mut pos = 0; let mut out = Vec::new();
    let perm = |st: &mut [u8; 200]| { let mut a = [0u64; 25];
        for i in 0..25 { let mut v = 0u64; for k in 0..nb { v |= (st[nb * i + k] as u64) << (8 * k); } a[i] = v; }
        keccak_p(&mut a, g.w, g.first, g.last);
        for i in 0..25 { for k in 0..nb { st[nb * i + k] = (a[i] >> (8 * k)) as u8; } } };
    for &b in msg { st[pos] ^= b; pos += 1; if pos == g.rate { perm(&mut st); pos = 0; } }
    st[pos] ^= dom; st[g.rate - 1] ^= 0x80;
    while out.len() < outlen { perm(&mut st); out.extend_from_slice(&st[..g.rate]); }
    out.truncate(outlen); out }
fn hx(v: &[u8]) -> String { v.iter().map(|b| format!("{:02x}", b)).collect::<String>() }
unsafe fn one_run(engs: &[Eng], used: usize, g: &Alg, dom: u8, ln: usize, olen: usize, packed: &[Vec<u8>], cnts: &[usize]) -> (f64, f64, u32, bool, Vec<Vec<u8>>) {
    let nb = g.w / 8; let dual = g.w == 32; let wpo = if dual { (olen + 3) / 4 } else { (olen + 7) / 8 };
    let te0 = now_ns();
    for a in 0..used { let e = &engs[a]; let (p, _) = e.buf(0);
        if !packed[a].is_empty() { std::ptr::copy_nonoverlapping(packed[a].as_ptr(), p, packed[a].len()); to_device(p, packed[a].len()); }
        e.wr(LEN, ln as u32 | ((dom as u32) << 24)); e.wr(OLEN, olen as u32); e.wr(ALG, g.first | (g.last << 8) | ((dual as u32) << 16) | ((g.rate as u32 / nb) << 24)); e.wr(0x44, 0); }
    let tr0 = now_ns();
    for a in 0..used { let e = &engs[a]; e.start(e.buf(0).1, e.buf(1).1, if dual { cnts[a] / 2 } else { cnts[a] }); }
    let mut ok = true; let mut pend = used; let mut done = vec![false; used]; let mut spins = 0u64;
    while pend > 0 { for a in 0..used { if !done[a] { if let Some(s) = engs[a].finished() { ok &= s & 0x72 == 2; done[a] = true; pend -= 1; } } }
        spins += 1; if spins > 3_000_000_000 { ok = false; break; } }
    let tr1 = now_ns(); let mut ck = 0u32; let mut outs = Vec::new();
    for a in 0..used { let e = &engs[a]; let nby = (if dual { cnts[a] / 2 } else { cnts[a] }) * wpo * 8; let (p, _) = e.buf(1); from_device(p, nby);
        let c = e.rd(CLOCKS); if c > ck { ck = c; } outs.push(std::slice::from_raw_parts(p, nby).to_vec()); }
    let te1 = now_ns(); (tr1 - tr0, te1 - te0, ck, ok, outs) }
// returns best run ns, best e2e ns, hardware clocks of the best run, mismatching digests over all runs, first mismatch
unsafe fn measure(engs: &[Eng], used: usize, g: &Alg, dom: u8, ln: usize, olen: usize, total: usize, msgs: &[u8], reps: usize) -> (f64, f64, u32, usize, String) {
    let dual = g.w == 32; let wpo = if dual { (olen + 3) / 4 } else { (olen + 7) / 8 };
    let want: Vec<Vec<u8>> = (0..total).map(|j| xofd(g, dom, &msgs[j * ln..(j + 1) * ln], olen)).collect();
    let cnts: Vec<usize> = (0..used).map(|a| total / used + ((a < total % used) as usize)).collect();
    let mut offs = vec![0usize; used]; for a in 1..used { offs[a] = offs[a - 1] + cnts[a - 1]; }
    let mut packed: Vec<Vec<u8>> = Vec::new();
    for a in 0..used { let c = cnts[a];
        if !dual { let wpm = (ln + 7) / 8; let mut o = vec![0u8; c * wpm * 8];
            for k in 0..c { let j = offs[a] + k; o[k * wpm * 8..k * wpm * 8 + ln].copy_from_slice(&msgs[j * ln..(j + 1) * ln]); } packed.push(o); }
        else { if c % 2 != 0 { return (0.0, 0.0, 0, total, "odd job count on a two-core atom".to_string()); }
            let wpm = (ln + 3) / 4; let mut o = vec![0u8; c / 2 * wpm * 8];
            for k in 0..c { let j = offs[a] + k; for i in 0..ln { o[(k / 2) * wpm * 8 + (i / 4) * 8 + (k % 2) * 4 + i % 4] = msgs[j * ln + i]; } } packed.push(o); } }
    for a in 0..used { if packed[a].len() > HALF || (if dual { cnts[a] / 2 } else { cnts[a] }) * wpo * 8 > HALF { return (0.0, 0.0, 0, total, "batch does not fit the 1 MB buffer half".to_string()); } }
    let mut best = f64::MAX; let mut beste = f64::MAX; let mut bck = 0u32; let mut bad = 0usize; let mut first = String::new();
    for _ in 0..reps { let (tr, te, ck, ok, outs) = one_run(engs, used, g, dom, ln, olen, &packed, &cnts); let mut m = 0usize;
        for a in 0..used { for k in 0..cnts[a] { let j = offs[a] + k; let o = &outs[a];
            let got: Vec<u8> = if !dual { o[k * wpo * 8..k * wpo * 8 + olen].to_vec() } else { (0..olen).map(|i| o[(k / 2) * wpo * 8 + (i / 4) * 8 + (k % 2) * 4 + i % 4]).collect() };
            if got != want[j] { m += 1; if first.is_empty() { let s = if olen > 40 { 40 } else { olen };
                first = format!("job={} atom={} len={} olen={} got={} want={} msg_first16={}", j, a, ln, olen, hx(&got[..s]), hx(&want[j][..s]), hx(&msgs[j * ln..j * ln + if ln > 16 { 16 } else { ln }])); } } } }
        if !ok { m += 1; if first.is_empty() { first = "status not clean (error flag or timeout)".to_string(); } }
        bad += m; if m == 0 && tr < best { best = tr; beste = te; bck = ck; } }
    (best, beste, bck, bad, first) }
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
    println!("RESULT apps engines={} engine_mhz={} ids={}", n, emhz, engs.iter().map(|e| format!("{:08x}", e.rd(ID))).collect::<Vec<String>>().join(" "));
    let m48: Vec<u8> = (0..48u8).collect(); let m100: Vec<u8> = (0..100u8).collect();
    println!("RESULT refvec minishake256 dom=01 msg=00..2f out32={}", hx(&xofd(&ALGS[5], 0x01, &m48, 32)));
    println!("RESULT refvec minishake128 dom=01 msg=00..63 out16={}", hx(&xofd(&ALGS[4], 0x01, &m100, 16)));
    let mut rng = Rng(0x1234_5678_9ABC_DEF1); let mut allbad = 0usize;
    // ---- MiniSHAKE256 has not run on a board before: shapes around the 36-byte rate first, on every atom ----
    for ei in 0..n { let mut cases = 0; let mut bad = 0; let mut first = String::new();
        for &(ln, olen, cnt) in [(0usize, 32usize, 4usize), (1, 32, 4), (35, 32, 4), (36, 32, 4), (37, 32, 4), (48, 32, 64), (48, 16, 8), (71, 32, 4), (72, 32, 4), (73, 64, 4), (100, 32, 8), (1000, 36, 4), (48, 200, 4), (3, 7, 2)].iter() {
            let msgs = rng.msg(ln * cnt); let (_, _, _, m, f) = measure(&engs[ei..ei + 1], 1, &ALGS[5], 0x01, ln, olen, cnt, &msgs, 1);
            cases += cnt; bad += m; if first.is_empty() && !f.is_empty() { first = f; } }
        allbad += bad; println!("RESULT minishake256_check atom={} digests={} mismatches={} first=[{}]", ei, cases, bad, first); }
    // ---- the application rows ----  name, function, domain, message bytes, output bytes, jobs, one job per atom
    let apps: [(&str, usize, u8, usize, usize, usize, bool); 10] = [
        ("mldsa_expand_16x840", 0, 0x1F, 34, 840, 16, false), ("mlkem_expand_9x504", 0, 0x1F, 34, 504, 9, false),
        ("mldsa_expand_1024x840", 0, 0x1F, 34, 840, 1024, false), ("mlkem_expand_576x504", 0, 0x1F, 34, 504, 576, false),
        ("firmware_1MB_turboshake128", 2, 0x1F, 1 << 20, 32, 1, true), ("firmware_1MB_shake128", 0, 0x1F, 1 << 20, 32, 1, true),
        ("log_200x232_turboshake256", 3, 0x1F, 232, 32, 200, false), ("log_4000x232_turboshake256", 3, 0x1F, 232, 32, 4000, false),
        ("tags_10000x100_minishake128", 4, 0x01, 100, 16, 10000, false), ("keys_10000x48_minishake256", 5, 0x01, 48, 32, 10000, false)];
    for &(name, gi, dom, ln, olen, cnt, per_atom) in apps.iter() { let g = &ALGS[gi];
        for &used in [1usize, 2, 4, 8].iter() { if used > n { continue; }
            let total = if per_atom { cnt * used } else { cnt }; let msgs = rng.msg(ln * total);
            let (tr, te, ck, bad, first) = measure(&engs, used, g, dom, ln, olen, total, &msgs, 5); allbad += bad;
            if tr == f64::MAX || tr == 0.0 { println!("RESULT app={} alg={} atoms={} jobs={} len={} olen={} FAILED mismatches={} first=[{}]", name, g.name, used, total, ln, olen, bad, first); continue; }
            let hwns = ck as f64 * 1000.0 / 300.0;
            println!("RESULT app={} alg={} atoms={} jobs={} len={} olen={} run_us={:.2} e2e_us={:.2} hw_clocks_max={} hw_us={:.2} run_ns_per_job={:.1} hw_ns_per_job={:.1} jobs_per_s={:.0} in_GB_per_s={:.3} out_GB_per_s={:.3} mismatches={} {}",
                name, g.name, used, total, ln, olen, tr / 1e3, te / 1e3, ck, hwns / 1e3, tr / total as f64, hwns / total as f64, total as f64 / (tr / 1e9),
                (total * ln) as f64 / tr, (total * olen) as f64 / tr, bad, if bad == 0 { "VERIFIED" } else { "FAILED" });
            if bad != 0 { println!("RESULT app={} atoms={} first_mismatch=[{}]", name, used, first); } } }
    println!("RESULT apps summary mismatches={} {}", allbad, if allbad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
