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
fn main() { unsafe {
    let args: Vec<String> = std::env::args().collect(); let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    if args.get(1).map(|s| s == "selftest").unwrap_or(false) {                                            // reference model only, no hardware
        let hx = |v: &[u8]| v.iter().map(|b| format!("{:02x}", b)).collect::<String>(); let m: Vec<u8> = (0..200u32).map(|i| (i * 7) as u8).collect();
        for g in ALGS.iter() { println!("{} {} {}", g.name, hx(&xof(g, &[], 16)), hx(&xof(g, &m, 40))); }
        let ms: Vec<Vec<u8>> = (0..4u8).map(|i| vec![i + 1; 7]).collect(); println!("pack {}", hx(&pack(&ALGS[4], &ms))); println!("unpack {:?}", unpack(&ALGS[4], &pack(&ALGS[4], &ms), 4, 7) == ms); return; }
    let bus: f64 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(300.0); let per = 1000.0 / bus;
    let fd = open(b"/dev/mem\0".as_ptr() as *const c_char, 2 | 0x101000); if fd < 0 { println!("RESULT error=open_dev_mem"); return; }
    let mut pm = File::open("/proc/self/pagemap").unwrap(); let mut engs = Vec::new(); let mut bad = 0usize;
    let hf = open(b"/dev/dma_heap/reserved\0".as_ptr() as *const c_char, 2); if hf < 0 { println!("RESULT error=open_dma_heap"); return; }
    for i in 0..n {
        let reg = mmap(std::ptr::null_mut(), 4096, 3, 1, fd, 0xA000_0000i64 + 0x1000 * i as i64) as *mut u32;
        let mut a = HeapAlloc { len: PAGE as u64, fd: 0, fd_flags: 2 | 0x80000, heap_flags: 0 };            // one contiguous 2 MB buffer
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
        let g = Eng { reg, va, pa };
        g.wr(WBASE, (pa >> 12) as u32); g.wr(WLIMIT, ((pa + PAGE as u64) >> 12) as u32);
        println!("RESULT engine={} id={:08x} buffer_pa={:#x}", i, g.rd(ID), pa); engs.push(g);
    }
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    // ---- 1. correctness on every engine: all six functions, several shapes ----
    for (ei, e) in engs.iter().enumerate() { let mut fail = 0; let mut checks = 0;
        for g in ALGS.iter() { for &(ln, olen, cnt) in [(0usize, 32usize, 4usize), (1, 32, 4), (32, 32, 64), (31, 64, 8), (67, 33, 8), (68, 200, 4), (135, 32, 4), (136, 32, 4), (167, 32, 4), (168, 500, 4), (169, 32, 4), (1000, 32, 4), (4096, 32, 4)].iter() {
            let msgs: Vec<Vec<u8>> = (0..cnt).map(|_| rng.msg(ln)).collect(); if ln > 0 { put(e, 0, &pack(g, &msgs)); }
            e.shape(g, ln, olen); let ok = e.run(e.buf(0).1, e.buf(1).1, jobs(g, cnt));
            let got = unpack(g, &get(e, 1, jobs(g, cnt) * owords(g, olen) * 8), cnt, olen);
            for k in 0..cnt { checks += 1; if !ok || got[k] != xof(g, &msgs[k], olen) { fail += 1; } } } }
        bad += fail; println!("RESULT check engine={} digests={} mismatches={}", ei, checks, fail); }
    let e = &engs[0];
    // ---- 2. time per hash, one engine ----
    for &(ai, ln, cnt) in [(0usize, 32usize, 20000usize), (0, 64, 16000), (0, 136, 6000), (0, 4096, 200), (1, 32, 20000), (2, 32, 20000), (2, 4096, 200), (4, 32, 20000), (4, 64, 16000), (5, 32, 20000)].iter() {
        let g = &ALGS[ai]; let base: Vec<Vec<u8>> = (0..16).map(|_| rng.msg(ln)).collect(); let msgs: Vec<Vec<u8>> = (0..cnt).map(|k| base[k % 16].clone()).collect();
        put(e, 0, &pack(g, &msgs)); e.shape(g, ln, 32); let t0 = now_ns(); let ok = e.run(e.buf(0).1, e.buf(1).1, jobs(g, cnt)); let t1 = now_ns(); let ck = e.rd(CLOCKS) as f64;
        let got = unpack(g, &get(e, 1, jobs(g, cnt) * owords(g, 32) * 8), cnt, 32); let f = (0..16).chain(cnt - 16..cnt).filter(|&k| got[k] != xof(g, &msgs[k], 32)).count() + (!ok) as usize; bad += f;
        println!("RESULT time alg={} len={} jobs={} hw_ns_per_hash={:.1} program_ns_per_hash={:.1} mismatches={}", g.name, ln, cnt, ck * per / cnt as f64, (t1 - t0) / cnt as f64, f); }
    // ---- 3. kernels of hash-based signatures, k engines at once (each engine runs one complete kernel) ----
    let mut k = 1; while k <= n {
        for &ai in [0usize, 2, 4].iter() { let g = &ALGS[ai]; let dual = g.w == 32;
            // hash chains: 67 chains (68 for two cores) of 15 steps, x = H(x)
            let nch = if dual { 68 } else { 67 }; let seeds: Vec<Vec<u8>> = (0..nch).map(|i| vec![i as u8; 32]).collect(); let reps = 200; let mut f = 0;
            for round in 0..2 { let steps = if round == 0 { 15 } else { 15 * reps };
                for x in engs[..k].iter() { put(x, 0, &pack(g, &seeds)); x.shape(g, 32, 32); }
                let t0 = now_ns(); let mut ok = true;
                for s in 0..steps { let (a, b) = (s % 2, 1 - s % 2); for x in engs[..k].iter() { x.start(x.buf(a).1, x.buf(b).1, jobs(g, nch)); } for x in engs[..k].iter() { ok &= x.wait() & 0x72 == 2; } }
                let t1 = now_ns();
                if round == 0 { let exp: Vec<Vec<u8>> = seeds.iter().map(|s| { let mut x = s.clone(); for _ in 0..15 { x = xof(g, &x, 32); } x }).collect();
                    for x in engs[..k].iter() { let got = unpack(g, &get(x, 1, jobs(g, nch) * owords(g, 32) * 8), nch, 32); f += (got != exp) as usize + (!ok) as usize; } bad += f; }
                else { println!("RESULT app=chains alg={} engines={} hashes_each={} ns_per_round_of_kernels={:.0} ns_per_kernel={:.0} mismatches={}", g.name, k, nch * 15, (t1 - t0) / reps as f64, (t1 - t0) / reps as f64 / k as f64, f); } }
            // Merkle tree, 256 leaves; two cores build the two half-trees side by side
            let leaves: Vec<Vec<u8>> = (0..256).map(|i| vec![i as u8; 32]).collect();
            let order: Vec<Vec<u8>> = if dual { (0..256).map(|i| leaves[i / 2 + if i % 2 == 1 { 128 } else { 0 }].clone()).collect() } else { leaves.clone() };
            let nl = if dual { 128 } else { 256 }; let mut f = 0;
            for round in 0..2 { let reps = if round == 0 { 1 } else { 200 };
                for x in engs[..k].iter() { put(x, 0, &pack(g, &order)); }
                let t0 = now_ns(); let mut ok = true; let mut last = 0;
                for _ in 0..reps { let mut s = 0; let mut cnt = nl; let mut first = true;
                    loop { let (a, b) = (s % 2, 1 - s % 2);
                        for x in engs[..k].iter() { if first || cnt == nl / 2 { x.shape(g, if first { 32 } else { 64 }, 32); } x.start(x.buf(a).1, x.buf(b).1, cnt); }
                        for x in engs[..k].iter() { ok &= x.wait() & 0x72 == 2; }
                        last = b; s += 1; if cnt == 1 { break; } cnt = if first { nl / 2 } else { cnt / 2 }; first = false; } }
                let t1 = now_ns();
                if round == 0 { let mut lvl: Vec<Vec<u8>> = leaves.iter().map(|x| xof(g, x, 32)).collect(); let stop = if dual { 2 } else { 1 };
                    while lvl.len() > stop { lvl = (0..lvl.len() / 2).map(|i| { let mut m = lvl[2 * i].clone(); m.extend_from_slice(&lvl[2 * i + 1]); xof(g, &m, 32) }).collect(); }
                    for x in engs[..k].iter() { let got = unpack(g, &get(x, last, jobs(g, stop) * owords(g, 32) * 8), stop, 32); f += (got != lvl) as usize + (!ok) as usize; } bad += f; }
                else { println!("RESULT app=merkle alg={} engines={} leaves=256 ns_per_round_of_kernels={:.0} ns_per_kernel={:.0} mismatches={}", g.name, k, (t1 - t0) / reps as f64, (t1 - t0) / reps as f64 / k as f64, f); } }
        }
        // ---- 4. the memory port: k engines hashing at once, time per hash of each engine from its own hardware counter ----
        for &(ai, ln, cnt) in [(0usize, 32usize, 20000usize), (2, 32, 20000), (4, 32, 20000), (0, 136, 6000), (0, 1024, 800), (0, 4096, 200), (2, 4096, 200)].iter() { let g = &ALGS[ai];
            let msgs: Vec<Vec<u8>> = (0..cnt).map(|i| vec![(i % 251) as u8; ln]).collect(); let p = pack(g, &msgs);
            for x in engs[..k].iter() { put(x, 0, &p); x.shape(g, ln, 32); }
            let t0 = now_ns(); for x in engs[..k].iter() { x.start(x.buf(0).1, x.buf(1).1, jobs(g, cnt)); }
            let mut ok = true; for (i, x) in engs[..k].iter().enumerate() { let st = x.wait(); if st & 0x72 != 2 { println!("RESULT port_error engine={} status={:08x}", i, st); } ok &= st & 0x72 == 2; } let t1 = now_ns();
            let each: Vec<String> = engs[..k].iter().map(|x| format!("{:.1}", x.rd(CLOCKS) as f64 * per / cnt as f64)).collect();
            let mean: f64 = engs[..k].iter().map(|x| x.rd(CLOCKS) as f64 * per / cnt as f64).sum::<f64>() / k as f64;
            let exp = xof(g, &msgs[cnt - 1], 32); let mut f = (!ok) as usize;
            for x in engs[..k].iter() { let got = unpack(g, &get(x, 1, jobs(g, cnt) * owords(g, 32) * 8), cnt, 32); f += (got[cnt - 1] != exp) as usize; } bad += f;
            println!("RESULT port alg={} len={} engines={} ns_per_hash_each={} mean={:.1} hashes_per_us_all={:.2} mismatches={}", g.name, ln, k, each.join("/"), mean, (cnt * k) as f64 / ((t1 - t0) / 1000.0), f); }
        k *= 2; }
    println!("RESULT summary mismatches={} {}", bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
