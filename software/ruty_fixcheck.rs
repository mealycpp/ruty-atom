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

// ================= ruty_fixcheck: fixed-width builds against the run-time atom, one program for every row =================
// One atom. The mode says what the loaded image can do; a function the image cannot do is never started.
//   rt : run-time atom            -> SHAKE256, TurboSHAKE256 (one Keccak-p[1600]) and MiniSHAKE128 (two Keccak-p[800])
//   w  : fixed 1x1600 (dual = 0)  -> SHAKE256, TurboSHAKE256
//   n  : fixed 2x800  (dual = 1)  -> MiniSHAKE128
// (a) one-shot hashes of a few shapes and chains (64-byte template, counter byte 47, 16-byte digest fed back at byte 48) against the
//     software reference above; (b) chain step time from the hardware clock counter (register 0x34, bus clocks at 300 MHz):
//     4,096 chains of 15 steps, best of 7, every digest of every run checked.
//   sudo ./ruty_fixcheck <rt|w|n> <engine MHz> <label>
const NCH: usize = 4096; const STEPS: usize = 15; const REPS: usize = 7; const BUS_MHZ: f64 = 300.0;
fn chain_ref_c(g: &Alg, dom: u8, tpl: &[u8], cend: u8) -> Vec<u8> { let mut m = tpl.to_vec();
    loop { let d = xofd(g, dom, &m, 16); m[48..64].copy_from_slice(&d); m[47] = m[47].wrapping_add(1); if m[47] == cend { return d; } } }
// dual_bit is what is WRITTEN to the width bit of the ALG register; the lane count of the rate is always the one of the function g.
unsafe fn set_shape(e: &Eng, g: &Alg, dual_bit: u32, dom: u8, len: usize, olen: usize, chain: Option<u8>) { let nb = g.w / 8;
    e.wr(LEN, len as u32 | ((dom as u32) << 24)); e.wr(OLEN, olen as u32); e.wr(ALG, g.first | (g.last << 8) | (dual_bit << 16) | ((g.rate as u32 / nb) << 24));
    match chain { Some(cend) => { e.wr(0x44, 1 | ((47 / nb) << 8) | ((47 % nb) << 16) | ((48 / nb) << 20)); e.wr(0x48, cend as u32); }
                  None => { e.wr(0x44, 0); } } }
// one batch of equal-shaped jobs on the atom: digests, hardware clocks, wall ns (start to finished), clean status
unsafe fn batch(e: &Eng, g: &Alg, dual_bit: u32, dom: u8, msgs: &[Vec<u8>], olen: usize, chain: Option<u8>) -> (Vec<Vec<u8>>, u32, f64, bool) {
    let n = msgs.len(); let data = pack(g, msgs); let nby = jobs(g, n) * owords(g, olen) * 8;
    if data.len() > HALF || nby > HALF { return (Vec::new(), 0, 0.0, false); }
    put(e, 0, &data); let z = vec![0u8; nby]; put(e, 1, &z);                    // the output half is cleared, so an old digest cannot pass
    set_shape(e, g, dual_bit, dom, msgs[0].len(), olen, chain);
    let t0 = now_ns(); let ok = e.run(e.buf(0).1, e.buf(1).1, jobs(g, n)); let t1 = now_ns();
    let ck = e.rd(CLOCKS); let raw = get(e, 1, nby); e.wr(0x44, 0);
    (unpack(g, &raw, n, olen), ck, t1 - t0, ok) }
fn main() { unsafe {
    let args: Vec<String> = std::env::args().collect(); let mode: String = args.get(1).cloned().unwrap_or_default();
    let emhz: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0); let label: String = args.get(3).cloned().unwrap_or_default();
    // (function index in ALGS, domain byte)
    let funcs: Vec<(usize, u8)> = if mode == "rt" { vec![(1, 0x1F), (3, 0x1F), (4, 0x01)] } else if mode == "w" { vec![(1, 0x1F), (3, 0x1F)] }
        else if mode == "n" { vec![(4, 0x01)] } else { println!("RESULT fixcheck error=mode_must_be_rt_w_or_n"); return; };
    let fd = open(b"/dev/mem\0".as_ptr() as *const c_char, 2 | 0x101000); if fd < 0 { println!("RESULT error=open_dev_mem"); return; }
    let mut pm = File::open("/proc/self/pagemap").unwrap();
    let hf = open(b"/dev/dma_heap/reserved\0".as_ptr() as *const c_char, 2); if hf < 0 { println!("RESULT error=open_dma_heap"); return; }
    let reg = mmap(std::ptr::null_mut(), 4096, 3, 1, fd, 0xA000_0000i64) as *mut u32;
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
    let e = Eng { reg, va, pa }; e.wr(WBASE, (pa >> 12) as u32); e.wr(WLIMIT, ((pa + PAGE as u64) >> 12) as u32);
    println!("RESULT fixcheck image={} mode={} engine_mhz={} id={:08x} functions={}", label, mode, emhz, e.rd(ID),
        funcs.iter().map(|f| ALGS[f.0].name).collect::<Vec<&str>>().join(","));
    let mut rng = Rng(0x0F1C_C4EC_2026_1126); let mut allbad = 0usize;
    for &(gi, dom) in funcs.iter() { let g = &ALGS[gi]; let dual = (g.w == 32) as u32;
        // ---- (a) one-shot hashes: lengths around the rate, long input, long output ----
        let r = g.rate; let shapes: Vec<(usize, usize, usize)> = vec![(0, 32, 4), (1, 32, 4), (r - 1, 32, 4), (r, 32, 4), (r + 1, 64, 4), (64, 16, 64),
            (100, 16, 16), (232, 32, 16), (2 * r, 32, 4), (1000, 200, 4), (34, 3 * r + 5, 4)];
        let (mut cases, mut bad, mut first) = (0usize, 0usize, String::new());
        for &(ln, olen, cnt) in shapes.iter() { let msgs: Vec<Vec<u8>> = (0..cnt).map(|_| rng.msg(ln)).collect();
            let (got, _, _, ok) = batch(&e, g, dual, dom, &msgs, olen, None); cases += cnt;
            if !ok || got.len() != cnt { bad += cnt; if first.is_empty() { first = format!("len={} olen={} status not clean", ln, olen); } continue; }
            for j in 0..cnt { let want = xofd(g, dom, &msgs[j], olen); if got[j] != want { bad += 1;
                if first.is_empty() { let s = if olen > 32 { 32 } else { olen }; first = format!("len={} olen={} job={} got={} want={}", ln, olen, j, hx(&got[j][..s]), hx(&want[..s])); } } } }
        allbad += bad; println!("RESULT fixcheck hash_check image={} alg={} digests={} mismatches={} first=[{}]", label, g.name, cases, bad, first);
        // ---- (a) chains: start value, end value, number of chains ----
        let (mut cases, mut bad) = (0usize, 0usize);
        for &(start, cend, cnt) in [(0u8, 15u8, 36usize), (0, 1, 4), (7, 15, 36), (14, 15, 4), (0, 15, 2), (3, 9, 16)].iter() {
            let msgs: Vec<Vec<u8>> = (0..cnt).map(|_| { let mut m = rng.msg(64); m[47] = start; m }).collect();
            let (got, _, _, ok) = batch(&e, g, dual, dom, &msgs, 16, Some(cend)); cases += cnt;
            if !ok || got.len() != cnt { bad += cnt; continue; }
            for j in 0..cnt { if got[j] != chain_ref_c(g, dom, &msgs[j], cend) { bad += 1; } } }
        allbad += bad; println!("RESULT fixcheck chain_check image={} alg={} chains={} mismatches={}", label, g.name, cases, bad);
        // ---- (b) chain step time: 4,096 different chains of 15 steps, best of 7 ----
        let msgs: Vec<Vec<u8>> = (0..NCH).map(|_| { let mut m = rng.msg(64); m[47] = 0; m }).collect();
        let want: Vec<Vec<u8>> = msgs.iter().map(|m| chain_ref_c(g, dom, m, STEPS as u8)).collect();
        let (mut best, mut worst, mut bwall, mut bad) = (u32::MAX, 0u32, f64::MAX, 0usize); let mut all = Vec::new();
        for _ in 0..REPS { let (got, ck, wall, ok) = batch(&e, g, dual, dom, &msgs, 16, Some(STEPS as u8)); let mut m = 0usize;
            if !ok || got.len() != NCH { m = NCH; } else { for j in 0..NCH { if got[j] != want[j] { m += 1; } } }
            bad += m; all.push(ck); if m == 0 { if ck < best { best = ck; } if ck > worst { worst = ck; } if wall < bwall { bwall = wall; } } }
        allbad += bad;
        if best == u32::MAX { println!("RESULT fixcheck chain_time image={} alg={} FAILED mismatches={}", label, g.name, bad); continue; }
        let ns = best as f64 * 1000.0 / BUS_MHZ; let steps = (NCH * STEPS) as f64;
        println!("RESULT fixcheck chain_time image={} alg={} chains={} steps_per_chain={} reps={} hw_clocks_best={} hw_clocks_worst={} hw_clocks_all={} step_ns_per_chain={:.3} ns_per_chain={:.2} Mhashes_per_s={:.3} Mchains_per_s={:.4} wall_us_best={:.1} mismatches={}",
            label, g.name, NCH, STEPS, REPS, best, worst, all.iter().map(|c| c.to_string()).collect::<Vec<String>>().join("/"), ns / steps, ns / NCH as f64,
            steps / ns * 1e3, NCH as f64 / ns * 1e3, bwall / 1e3, bad); }
    // ---- fixed builds only: the width bit of the register must be dead. The supported function is asked for with the bit written the
    //      WRONG way; a fixed build ignores the bit and still returns the right digest (the run-time atom would not). ----
    if mode == "w" || mode == "n" { let (gi, dom) = funcs[0]; let g = &ALGS[gi]; let wrong = 1 - (g.w == 32) as u32;
        let msgs: Vec<Vec<u8>> = (0..8).map(|_| rng.msg(64)).collect(); let (got, _, _, ok) = batch(&e, g, wrong, dom, &msgs, 16, None); let mut bad = 0usize;
        if !ok || got.len() != 8 { bad = 8; } else { for j in 0..8 { if got[j] != xofd(g, dom, &msgs[j], 16) { bad += 1; } } }
        allbad += bad; println!("RESULT fixcheck width_bit_ignored image={} alg={} width_bit_written={} digests=8 mismatches={} {}", label, g.name, wrong, bad,
            if bad == 0 { "BIT_IS_DEAD_IN_HARDWARE" } else { "BIT_STILL_ACTS" }); }
    e.wr(0x44, 0);
    println!("RESULT fixcheck summary image={} mismatches={} {}", label, allbad, if allbad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
