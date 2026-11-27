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

// ================= ruty_tenants: two unlike tenants on the same atoms at the same time =================
// Tenant S: SHAKE256 hash chains (64-byte template, counter in byte 47, 16-byte digest fed back at byte 48, 15 steps) on WIDE atoms.
// Tenant T: MiniSHAKE128 tags, 100 bytes -> 16 bytes, domain 0x01, on SPLIT atoms (two 800-bit cores per atom).
// A formation says, for every atom, which tenant it serves. It is set by register writes only; the image is never reloaded.
//   alone  : one tenant on k atoms, the other atoms idle
//   shared : S on k atoms and T on the other n-k atoms, started together; T is restarted until S has finished, so S is overlapped all the time
//   reform : the formation changes every round, so the same atom is wide in one job and split in the next
// Every digest of every run is compared with the software reference.      sudo ./ruty_tenants <atoms in the image> [engine MHz]
const NS: usize = 6144; const NT: usize = 9600;
fn chain_ref(g: &Alg, dom: u8, tpl: &[u8]) -> Vec<u8> { let mut m = tpl.to_vec();
    loop { let d = xofd(g, dom, &m, 16); m[48..64].copy_from_slice(&d); m[47] = m[47].wrapping_add(1); if m[47] == 15 { return d; } } }
struct Work { s_in: Vec<u8>, s_ref: Vec<Vec<u8>>, t_in: Vec<u8>, t_ref: Vec<Vec<u8>> }
#[derive(Clone, Copy)] struct Slot { role: u8, off: usize, cnt: usize }            // role: 0 idle, 1 tenant S (wide), 2 tenant T (split)
fn slots(roles: &[u8]) -> Vec<Slot> { let ks = roles.iter().filter(|&&r| r == 1).count(); let kt = roles.iter().filter(|&&r| r == 2).count();
    let (mut so, mut to) = (0usize, 0usize); let mut v = Vec::new();
    for &r in roles { if r == 1 { v.push(Slot { role: 1, off: so, cnt: NS / ks }); so += NS / ks; } else if r == 2 { v.push(Slot { role: 2, off: to, cnt: NT / kt }); to += NT / kt; }
        else { v.push(Slot { role: 0, off: 0, cnt: 0 }); } } v }
unsafe fn load(e: &Eng, w: &Work, s: &Slot) { let (p, _) = e.buf(0);
    if s.role == 1 { let nby = s.cnt * 64; std::ptr::copy_nonoverlapping(w.s_in[s.off * 64..].as_ptr(), p, nby); to_device(p, nby); }
    else if s.role == 2 { let nby = s.cnt / 2 * 200; let o = std::slice::from_raw_parts_mut(p, nby);
        for k in 0..s.cnt { let j = s.off + k; for i in 0..100 { o[(k / 2) * 200 + (i / 4) * 8 + (k % 2) * 4 + i % 4] = w.t_in[j * 100 + i]; } } to_device(p, nby); } }
unsafe fn shape(e: &Eng, role: u8) {
    if role == 1 { let g = &ALGS[1]; e.wr(LEN, 64 | (0x1F << 24)); e.wr(OLEN, 16); e.wr(ALG, g.first | (g.last << 8) | ((g.rate as u32 / 8) << 24));
        e.wr(0x44, 1 | ((47 / 8) << 8) | ((47 % 8) << 16) | ((48 / 8) << 20)); e.wr(0x48, 15); }
    else if role == 2 { let g = &ALGS[4]; e.wr(LEN, 100 | (0x01 << 24)); e.wr(OLEN, 16); e.wr(ALG, g.first | (g.last << 8) | (1 << 16) | ((g.rate as u32 / 4) << 24)); e.wr(0x44, 0); } }
unsafe fn go(e: &Eng, s: &Slot) { e.start(e.buf(0).1, e.buf(1).1, if s.role == 2 { s.cnt / 2 } else { s.cnt }); }
unsafe fn check(e: &Eng, w: &Work, s: &Slot) -> usize { let (p, _) = e.buf(1); let mut bad = 0;
    if s.role == 1 { let nby = s.cnt * 16; from_device(p, nby); let o = std::slice::from_raw_parts(p, nby);
        for k in 0..s.cnt { if o[k * 16..k * 16 + 16] != w.s_ref[s.off + k][..] { bad += 1; } } }
    else if s.role == 2 { let nby = s.cnt / 2 * 32; from_device(p, nby); let o = std::slice::from_raw_parts(p, nby);
        for k in 0..s.cnt { for i in 0..16 { if o[(k / 2) * 32 + (i / 4) * 8 + (k % 2) * 4 + i % 4] != w.t_ref[s.off + k][i] { bad += 1; break; } } } }
    bad }
struct Run { ts: f64, tt: f64, cks: u32, ckt: u32, tbatches: usize, bad: usize, shape_ns: f64, load_ns: f64 }
// one formation: load the data, shape the atoms, start all, wait. ts = last S atom done, tt = every T atom done once.
unsafe fn run(engs: &[Eng], w: &Work, roles: &[u8], repeat: bool) -> Run { let n = roles.len(); let sl = slots(roles);
    let l0 = now_ns(); for a in 0..n { load(&engs[a], w, &sl[a]); } let l1 = now_ns();
    for a in 0..n { shape(&engs[a], sl[a].role); } let l2 = now_ns();
    let mut spend = sl.iter().filter(|s| s.role == 1).count(); let mut tfirst = sl.iter().filter(|s| s.role == 2).count();
    let mut running: Vec<bool> = sl.iter().map(|s| s.role != 0).collect(); let mut first = vec![false; n]; let mut left = running.iter().filter(|&&r| r).count();
    let (mut ts, mut tt, mut cks, mut ckt, mut tb, mut ok, mut spins) = (0.0f64, 0.0f64, 0u32, 0u32, 0usize, true, 0u64);
    let t0 = now_ns(); for a in 0..n { if sl[a].role != 0 { go(&engs[a], &sl[a]); } }
    while left > 0 { for a in 0..n { if !running[a] { continue; } if let Some(s) = engs[a].finished() { ok &= s & 0x72 == 2;
            if sl[a].role == 1 { let c = engs[a].rd(CLOCKS); if c > cks { cks = c; } running[a] = false; left -= 1; spend -= 1; if spend == 0 { ts = now_ns() - t0; } }
            else { tb += 1; if !first[a] { first[a] = true; let c = engs[a].rd(CLOCKS); if c > ckt { ckt = c; } tfirst -= 1; if tfirst == 0 { tt = now_ns() - t0; } }
                if repeat && spend > 0 { go(&engs[a], &sl[a]); } else { running[a] = false; left -= 1; } } } }
        spins += 1; if spins > 3_000_000_000 { ok = false; break; } }
    let mut bad = (!ok) as usize; for a in 0..n { bad += check(&engs[a], w, &sl[a]); }
    Run { ts, tt, cks, ckt, tbatches: tb, bad, shape_ns: l2 - l1, load_ns: l1 - l0 } }
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
    println!("RESULT tenants atoms={} engine_mhz={} ids={} S=shake256_chains_x15 NS={} T=minishake128_tags_100to16 NT={}", n, emhz,
        engs.iter().map(|e| format!("{:08x}", e.rd(ID))).collect::<Vec<String>>().join(" "), NS, NT);
    let mut rng = Rng(0x0BAD_5EED_1234_5677); let mut s_in = rng.msg(NS * 64); for j in 0..NS { s_in[j * 64 + 47] = 0; } let t_in = rng.msg(NT * 100);
    let s_ref: Vec<Vec<u8>> = (0..NS).map(|j| chain_ref(&ALGS[1], 0x1F, &s_in[j * 64..j * 64 + 64])).collect();
    let t_ref: Vec<Vec<u8>> = (0..NT).map(|j| xofd(&ALGS[4], 0x01, &t_in[j * 100..j * 100 + 100], 16)).collect();
    let w = Work { s_in, s_ref, t_in, t_ref }; let mut allbad = 0usize; const REPS: usize = 7;
    let mut s_alone = vec![0.0f64; n + 1]; let mut t_alone = vec![0.0f64; n + 1];
    for k in 1..=n { if NS % k != 0 { continue; } let roles: Vec<u8> = (0..n).map(|i| (i < k) as u8).collect(); let mut best = f64::MAX; let mut ck = 0; let mut bad = 0;
        for _ in 0..REPS { let r = run(&engs, &w, &roles, false); bad += r.bad; if r.bad == 0 && r.ts < best { best = r.ts; ck = r.cks; } } allbad += bad; s_alone[k] = best;
        println!("RESULT tenants alone tenant=S atoms={} time_us={:.1} hw_us={:.1} Mchains_per_s={:.3} mismatches={}", k, best / 1e3, ck as f64 / 300.0, NS as f64 / best * 1e3, bad); }
    for m in 1..=n { if NT % (2 * m) != 0 { continue; } let roles: Vec<u8> = (0..n).map(|i| if i < m { 2 } else { 0 }).collect(); let mut best = f64::MAX; let mut ck = 0; let mut bad = 0;
        for _ in 0..REPS { let r = run(&engs, &w, &roles, false); bad += r.bad; if r.bad == 0 && r.tt < best { best = r.tt; ck = r.ckt; } } allbad += bad; t_alone[m] = best;
        println!("RESULT tenants alone tenant=T atoms={} time_us={:.1} hw_us={:.1} Mtags_per_s={:.3} mismatches={}", m, best / 1e3, ck as f64 / 300.0, NT as f64 / best * 1e3, bad); }
    for k in 1..n { let m = n - k; if NS % k != 0 || NT % (2 * m) != 0 { continue; } let roles: Vec<u8> = (0..n).map(|i| if i < k { 1 } else { 2 }).collect();
        let (mut bs, mut bt, mut bad, mut tb, mut cs, mut ct) = (f64::MAX, f64::MAX, 0usize, 0usize, 0u32, 0u32);
        for _ in 0..REPS { let r = run(&engs, &w, &roles, true); bad += r.bad; if r.bad == 0 { if r.ts < bs { bs = r.ts; tb = r.tbatches; cs = r.cks; } if r.tt < bt { bt = r.tt; ct = r.ckt; } } } allbad += bad;
        println!("RESULT tenants shared S_atoms={} T_atoms={} S_time_us={:.1} S_hw_us={:.1} S_alone_us={:.1} S_vs_alone={:.4} T_first_pass_us={:.1} T_hw_us={:.1} T_alone_us={:.1} T_vs_alone={:.4} T_batches_while_S={} T_Mtags_per_s_over_S_window={:.3} mismatches={}",
            k, m, bs / 1e3, cs as f64 / 300.0, s_alone[k] / 1e3, bs / s_alone[k], bt / 1e3, ct as f64 / 300.0, t_alone[m] / 1e3, bt / t_alone[m], tb, (tb * (NT / m)) as f64 / bs * 1e3, bad); }
    // ---- re-formation: the formation changes every round ----
    if n >= 2 { let rounds = 60usize; let mut prev = vec![0u8; n]; let (mut changes, mut bad, mut shp, mut ld, mut w2s, mut s2w) = (0usize, 0usize, 0.0f64, 0.0f64, 0usize, 0usize);
        for r in 0..rounds { let k = 1 + r % (n - 1); if NS % k != 0 || NT % (2 * (n - k)) != 0 { continue; }
            let roles: Vec<u8> = (0..n).map(|i| if (i + r) % n < k { 1 } else { 2 }).collect();
            for i in 0..n { if prev[i] == 1 && roles[i] == 2 { changes += 1; w2s += 1; } if prev[i] == 2 && roles[i] == 1 { changes += 1; s2w += 1; } }
            let x = run(&engs, &w, &roles, false); bad += x.bad; shp += x.shape_ns; ld += x.load_ns; prev = roles; }
        allbad += bad;
        println!("RESULT tenants reform rounds={} width_changes={} wide_to_split={} split_to_wide={} mismatches={} shape_ns_per_atom={:.0} load_us_per_formation={:.1}",
            rounds, changes, w2s, s2w, bad, shp / (rounds * n) as f64, ld / rounds as f64 / 1e3); }
    for e in engs.iter() { e.wr(0x44, 0); }
    println!("RESULT tenants summary mismatches={} {}", allbad, if allbad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
