// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_modes2.rs (NEW, 2026-11-26): ruty_modes.rs with one change, so that every median can be recomputed from the individual runs:
// fn stats() also prints samples_us=<the n times, in run order>. Nothing else differs from ruty_modes.rs.
// ruty_modes.rs (NEW FILE; nothing existing was changed): the NATIVE application of each hash mode of RUTY-ATOM, run on the atoms.
//   kt     : KangarooTwelve KT128 (RFC 9861), the scheme TurboSHAKE128 exists for. 8192-byte leaves (TurboSHAKE128, D=0x0B, 32-byte CV)
//            spread over the atoms and read IN PLACE (no copy); the atoms write the CVs straight into the final-node buffer; the final
//            node (TurboSHAKE128, D=0x06; single node D=0x07) runs on atom 0. Also flat SHAKE128 / TurboSHAKE128 of the same message.
//   merkle : binary Merkle tree over 2^k 32-byte values; leaf = H(value), node = H(left || right), 32-byte outputs.
//            H = MiniSHAKE128 (split atoms, two Keccak-p[800] per atom; NON-STANDARD tree hash), TurboSHAKE128 (wide; NON-STANDARD tree
//            hash) or SHAKE128 (wide). Each level is hashed IN PLACE: the outputs of a level are the inputs of the next one, no copy.
//   sudo ./ruty_modes <atoms in image> <engine MHz> <kt|merkle> <repetitions> [k list for merkle, e.g. 12,16,20]
// Built for aarch64 it drives the board; built anywhere else the atoms are replaced by an emulator of atom_seq3.v (non-chain jobs only)
// so the data layout can be checked against the software reference before any board run.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::ptr::{read_volatile, write_volatile};
#[repr(C)] struct Timespec { tv_sec: c_long, tv_nsec: c_long }
extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) -> *mut c_void;
    fn clock_gettime(clk: c_int, ts: *mut Timespec) -> c_int;
}
#[cfg(target_arch = "aarch64")] extern "C" { fn ruty_dc_cvac(a: usize); fn ruty_dc_civac(a: usize); fn ruty_dsb(); }
#[cfg(not(target_arch = "aarch64"))] unsafe fn ruty_dc_cvac(_a: usize) {}
#[cfg(not(target_arch = "aarch64"))] unsafe fn ruty_dc_civac(_a: usize) {}
#[cfg(not(target_arch = "aarch64"))] unsafe fn ruty_dsb() {}
const CTRL: usize = 0x00; const STATUS: usize = 0x04; const IN_LO: usize = 0x08; const IN_HI: usize = 0x0C; const OUT_LO: usize = 0x10;
const OUT_HI: usize = 0x14; const NJOBS: usize = 0x18; const LEN: usize = 0x1C; const OLEN: usize = 0x20; const ALG: usize = 0x24;
const WBASE: usize = 0x28; const WLIMIT: usize = 0x2C; const ID: usize = 0x38;
const PAGE: usize = 2 << 20;
fn now_ns() -> f64 { let mut t = Timespec { tv_sec: 0, tv_nsec: 0 }; unsafe { clock_gettime(1, &mut t); } t.tv_sec as f64 * 1e9 + t.tv_nsec as f64 }

// ---------------- software Keccak-p with 64-bit or 32-bit lanes (as in ruty_sign4.rs, round constants from a table) ----------------
const RHO: [u32; 25] = [0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14];
fn rc(r: u32) -> u64 { let mut l: u32 = 1; let mut v = 0u64;
    for _ in 0..r { for _ in 0..7 { l = (l << 1) ^ (if l & 0x80 != 0 { 0x171 } else { 0 }); } }
    for j in 0..7 { if l & 1 != 0 { v |= 1u64 << ((1u32 << j) - 1); } l = (l << 1) ^ (if l & 0x80 != 0 { 0x171 } else { 0 }); } v }
static mut RCT: [u64; 24] = [0u64; 24];
fn rol(x: u64, n: u32, w: u32) -> u64 { let n = n % w; let m = if w == 64 { !0u64 } else { 0xffff_ffff }; if n == 0 { x } else { ((x << n) | (x >> (w - n))) & m } }
fn keccak_p(a: &mut [u64; 25], w: u32, first: u32, last: u32) { let m = if w == 64 { !0u64 } else { 0xffff_ffff };
    for r in first..=last {
        let mut c = [0u64; 5]; for x in 0..5 { c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20]; }
        for i in 0..25 { a[i] ^= c[(i % 5 + 4) % 5] ^ rol(c[(i % 5 + 1) % 5], 1, w); }
        let mut b = [0u64; 25]; for x in 0..5 { for y in 0..5 { b[y + 5 * ((2 * x + 3 * y) % 5)] = rol(a[x + 5 * y], RHO[x + 5 * y], w); } }
        for i in 0..25 { a[i] = b[i] ^ (!b[(i % 5 + 1) % 5 + 5 * (i / 5)] & m & b[(i % 5 + 2) % 5 + 5 * (i / 5)]); }
        a[0] ^= unsafe { RCT[r as usize] } & m; } }
#[derive(Clone, Copy)] struct Alg { name: &'static str, w: u32, rate: usize, first: u32, last: u32 }
const ALGS: [Alg; 6] = [Alg { name: "shake128", w: 64, rate: 168, first: 0, last: 23 }, Alg { name: "shake256", w: 64, rate: 136, first: 0, last: 23 },
    Alg { name: "turboshake128", w: 64, rate: 168, first: 12, last: 23 }, Alg { name: "turboshake256", w: 64, rate: 136, first: 12, last: 23 },
    Alg { name: "minishake128", w: 32, rate: 68, first: 10, last: 21 }, Alg { name: "minishake256", w: 32, rate: 36, first: 10, last: 21 }];
const S128: usize = 0; const T128: usize = 2; const M128: usize = 4;
fn xofd(g: &Alg, dom: u8, msg: &[u8], outlen: usize) -> Vec<u8> { let nb = (g.w / 8) as usize; let mut st = [0u8; 200]; let mut out = Vec::with_capacity(outlen + 200);
    let perm = |st: &mut [u8; 200]| { let mut a = [0u64; 25];
        for i in 0..25 { let mut v = 0u64; for k in 0..nb { v |= (st[nb * i + k] as u64) << (8 * k); } a[i] = v; }
        keccak_p(&mut a, g.w, g.first, g.last);
        for i in 0..25 { for k in 0..nb { st[nb * i + k] = (a[i] >> (8 * k)) as u8; } } };
    let mut i = 0; while i + g.rate <= msg.len() { for j in 0..g.rate { st[j] ^= msg[i + j]; } perm(&mut st); i += g.rate; }
    let rest = msg.len() - i; for j in 0..rest { st[j] ^= msg[i + j]; }
    st[rest] ^= dom; st[g.rate - 1] ^= 0x80;
    while out.len() < outlen { perm(&mut st); out.extend_from_slice(&st[..g.rate]); }
    out.truncate(outlen); out }
fn hx(b: &[u8]) -> String { let mut s = String::with_capacity(b.len() * 2); for x in b { s.push_str(&format!("{:02x}", x)); } s }
fn unhx(s: &str) -> Vec<u8> { let c: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect(); (0..c.len() / 2).map(|i| u8::from_str_radix(&c[2 * i..2 * i + 2].iter().collect::<String>(), 16).unwrap()).collect() }

// ---------------- one atom ----------------
struct Eng { reg: *mut u32, pa: u64 }
impl Eng {
    unsafe fn wr(&self, off: usize, v: u32) { write_volatile(self.reg.add(off / 4), v) }
    unsafe fn rd(&self, off: usize) -> u32 { read_volatile(self.reg.add(off / 4)) }
    unsafe fn start(&self, inp: u64, out: u64, njobs: usize) { self.wr(IN_LO, inp as u32); self.wr(IN_HI, (inp >> 32) as u32); self.wr(OUT_LO, out as u32);
        self.wr(OUT_HI, (out >> 32) as u32); self.wr(NJOBS, njobs as u32); self.wr(CTRL, 2); self.wr(CTRL, 1);
        #[cfg(not(target_arch = "aarch64"))] emu(self); }
    unsafe fn finished(&self) -> Option<u32> { let s = self.rd(STATUS); if s & 1 == 0 && s & 0x42 != 0 { Some(s) } else { None } }
}
unsafe fn to_device(p: *mut u8, n: usize) { let mut a = p as usize & !63; while a < p as usize + n { ruty_dc_cvac(a); a += 64; } ruty_dsb(); }
unsafe fn from_device(p: *mut u8, n: usize) { let mut a = p as usize & !63; while a < p as usize + n { ruty_dc_civac(a); a += 64; } ruty_dsb(); }
struct Rng(u64); impl Rng { fn byte(&mut self) -> u8 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 24) as u8 }
    fn msg(&mut self, n: usize) -> Vec<u8> { (0..n).map(|_| self.byte()).collect() } }
#[repr(C)] struct HeapAlloc { len: u64, fd: u32, fd_flags: u32, heap_flags: u64 }
extern "C" { fn ioctl(fd: c_int, req: u64, arg: *mut HeapAlloc) -> c_int; }
const LIMIT: u64 = 0x8000_0000;                          // the image maps the low 2 GB of memory to the atoms
unsafe fn win(e: &Eng, pa: u64, len: usize) { let lo = if pa < e.pa { pa } else { e.pa }; let h1 = pa + len as u64; let h2 = e.pa + PAGE as u64; let hi = if h1 > h2 { h1 } else { h2 };
    e.wr(WBASE, (lo >> 12) as u32); e.wr(WLIMIT, ((hi + 4095) >> 12) as u32); }
/// one batch: nj equal jobs, inputs at stride ceil(len/w)*8 bytes, outputs at stride ceil(olen/w)*8 bytes (w = 8 wide, 4 split)
unsafe fn go(e: &Eng, g: &Alg, dom: u8, len: usize, olen: usize, inp: u64, out: u64, nj: usize) {
    assert!(len < (1 << 24), "LEN field is 24 bits"); let dual = g.w == 32; let nb = g.w / 8;
    e.wr(LEN, len as u32 | ((dom as u32) << 24)); e.wr(OLEN, olen as u32);
    e.wr(ALG, g.first | (g.last << 8) | ((dual as u32) << 16) | ((g.rate as u32 / nb) << 24)); e.wr(0x44, 0); e.start(inp, out, nj); }
unsafe fn wait1(e: &Eng) -> bool { let mut n = 0u64; loop { if let Some(s) = e.finished() { return s & 0x72 == 2; } n += 1; if n > 3_000_000_000 { return false; } } }

// ---------------- emulator of atom_seq3.v, non-chain jobs (only when NOT built for the board) ----------------
#[cfg(not(target_arch = "aarch64"))]
unsafe fn emu(e: &Eng) {
    let lr = e.rd(LEN); let len = (lr & 0xFF_FFFF) as usize; let dom = (lr >> 24) as u8; let olen = e.rd(OLEN) as usize; let alg = e.rd(ALG);
    let (first, last, dual, rl) = (alg & 31, (alg >> 8) & 31, (alg >> 16) & 1 == 1, ((alg >> 24) & 31) as usize); let nj = e.rd(NJOBS) as usize;
    let inp = (e.rd(IN_LO) as u64 | ((e.rd(IN_HI) as u64) << 32)) as usize as *const u8; let out = (e.rd(OUT_LO) as u64 | ((e.rd(OUT_HI) as u64) << 32)) as usize as *mut u8;
    let nb = if dual { 4 } else { 8 }; let wpm = (len + nb - 1) / nb; let wpo = (olen + nb - 1) / nb; let w = if dual { 32 } else { 64 };
    let g = Alg { name: "emu", w: w, rate: rl * nb, first: first, last: last }; let nc = if dual { 2 } else { 1 };
    for j in 0..nj { let src = inp.add(j * wpm * 8); let dst = out.add(j * wpo * 8);
        // the atom reads a job completely before it writes its digest: emulate that order (matters for in-place levels)
        let msgs: Vec<Vec<u8>> = (0..nc).map(|h| (0..len).map(|i| if dual { *src.add((i / 4) * 8 + h * 4 + i % 4) } else { *src.add(i) }).collect()).collect();
        for h in 0..nc { let d = xofd(&g, dom, &msgs[h], olen);
            for i in 0..olen { if dual { *dst.add((i / 4) * 8 + h * 4 + i % 4) = d[i]; } else { *dst.add(i) = d[i]; } } } }
    e.wr(STATUS, 2); }

// ---------------- buffers ----------------
struct Big { va: *mut u8, pa: u64, cap: usize }
#[cfg(target_arch = "aarch64")]
unsafe fn heap_buf(hf: c_int, pm: &mut File, len: usize) -> Option<(*mut u8, u64)> {
    let mut a = HeapAlloc { len: len as u64, fd: 0, fd_flags: 2 | 0x80000, heap_flags: 0 };
    if ioctl(hf, 0xC018_4800, &mut a as *mut HeapAlloc) != 0 { return None; }
    let va = mmap(std::ptr::null_mut(), len, 3, 1, a.fd as c_int, 0) as *mut u8; if va as isize == -1 { return None; }
    let mut k = 0; while k < len { *va.add(k) = 0; k += 4096; }
    let mut pf = [0u64; 2];
    for (j, off) in [0usize, len - 4096].iter().enumerate() { let mut e = [0u8; 8]; pm.seek(SeekFrom::Start(((va as u64 + *off as u64) / 4096) * 8)).unwrap(); pm.read_exact(&mut e).unwrap();
        let ent = u64::from_le_bytes(e); if ent >> 63 == 0 || ent & ((1 << 55) - 1) == 0 { return None; } pf[j] = ent & ((1 << 55) - 1); }
    let pa = pf[0] * 4096; if pf[1] - pf[0] != (len as u64 / 4096) - 1 || pa + len as u64 > LIMIT { return None; }
    Some((va, pa)) }
#[cfg(target_arch = "aarch64")]
unsafe fn setup(n: usize) -> Option<(Vec<Eng>, c_int, File)> {
    let fd = open(b"/dev/mem\0".as_ptr() as *const c_char, 2 | 0x101000); if fd < 0 { println!("RESULT error=open_dev_mem"); return None; }
    let mut pm = File::open("/proc/self/pagemap").unwrap();
    let hf = open(b"/dev/dma_heap/reserved\0".as_ptr() as *const c_char, 2); if hf < 0 { println!("RESULT error=open_dma_heap"); return None; }
    let mut engs = Vec::new();
    for i in 0..n { let reg = mmap(std::ptr::null_mut(), 4096, 3, 1, fd, 0xA000_0000i64 + 0x1000 * i as i64) as *mut u32;
        let (_va, pa) = match heap_buf(hf, &mut pm, PAGE) { Some(x) => x, None => { println!("RESULT error=atom_buffer"); return None; } };
        let g = Eng { reg: reg, pa: pa }; g.wr(WBASE, (pa >> 12) as u32); g.wr(WLIMIT, ((pa + PAGE as u64) >> 12) as u32); g.wr(0x44, 0); engs.push(g); }
    Some((engs, hf, pm)) }
#[cfg(target_arch = "aarch64")]
unsafe fn big(hf: c_int, pm: &mut File, cap: usize) -> Option<Big> { let cap = (cap + 4095) & !4095; match heap_buf(hf, pm, cap) { Some((va, pa)) => Some(Big { va: va, pa: pa, cap: cap }), None => None } }
#[cfg(not(target_arch = "aarch64"))]
unsafe fn setup(n: usize) -> Option<(Vec<Eng>, c_int, File)> {
    let mut engs = Vec::new(); for _ in 0..n { let reg = Box::leak(vec![0u32; 64].into_boxed_slice()).as_mut_ptr(); engs.push(Eng { reg: reg, pa: 0 }); }
    Some((engs, 0, File::open("/dev/null").unwrap())) }
#[cfg(not(target_arch = "aarch64"))]
unsafe fn big(_hf: c_int, _pm: &mut File, cap: usize) -> Option<Big> { let cap = (cap + 4095) & !4095; let v = Box::leak(vec![0u8; cap + 64].into_boxed_slice()).as_mut_ptr();
    let va = ((v as usize + 63) & !63) as *mut u8; Some(Big { va: va, pa: va as u64, cap: cap }) }

fn stats(v: &[f64]) -> (f64, String) { let mut s = v.to_vec(); s.sort_by(|a, b| a.partial_cmp(b).unwrap()); let n = s.len();
    // ruty_modes2: the only change from ruty_modes is the field samples_us, the n individual times in the order they were taken
    let all: Vec<String> = v.iter().map(|x| format!("{:.2}", x / 1e3)).collect();
    (s[n / 2], format!("n={} min_us={:.2} q1_us={:.2} median_us={:.2} q3_us={:.2} max_us={:.2} spread_max_over_min={:.4} samples_us={}", n, s[0] / 1e3, s[n / 4] / 1e3, s[n / 2] / 1e3, s[(3 * n) / 4] / 1e3, s[n - 1] / 1e3, s[n - 1] / s[0], all.join(","))) }
fn med(v: &[f64]) -> f64 { let mut s = v.to_vec(); s.sort_by(|a, b| a.partial_cmp(b).unwrap()); s[s.len() / 2] }

// ======================= KangarooTwelve KT128 (RFC 9861) =======================
const KTC: usize = 8192;
fn lenc(x: usize) -> Vec<u8> { let mut v = Vec::new(); let mut y = x; while y > 0 { v.insert(0, (y & 255) as u8); y >>= 8; } let n = v.len() as u8; v.push(n); v }
fn kt_s(m: &[u8], c: &[u8]) -> Vec<u8> { let mut s = m.to_vec(); s.extend_from_slice(c); s.extend_from_slice(&lenc(c.len())); s }
fn kt128_sw(s: &[u8], olen: usize) -> Vec<u8> { let t = &ALGS[T128];
    if s.len() <= KTC { return xofd(t, 0x07, s, olen); }
    let n = (s.len() + KTC - 1) / KTC; let mut node = s[..KTC].to_vec(); node.extend_from_slice(&[3, 0, 0, 0, 0, 0, 0, 0]);
    for i in 1..n { let e = if (i + 1) * KTC < s.len() { (i + 1) * KTC } else { s.len() }; node.extend_from_slice(&xofd(t, 0x0B, &s[i * KTC..e], 32)); }
    node.extend_from_slice(&lenc(n - 1)); node.extend_from_slice(&[0xFF, 0xFF]); xofd(t, 0x06, &node, olen) }
fn ptn(n: usize) -> Vec<u8> { (0..n).map(|i| (i % 251) as u8).collect() }
const KT_FO: usize = 25 << 20; const KT_OO: usize = KT_FO + (192 << 10); const KT_CAP: usize = KT_FO + (256 << 10);
unsafe fn put_s(b: &Big, s: &[u8]) { assert!(s.len() + 64 <= KT_FO); std::ptr::copy_nonoverlapping(s.as_ptr(), b.va, s.len()); std::ptr::write_bytes(b.va.add(s.len()), 0, 64); }
struct KtT { run: f64, leaves: f64, fin: f64, s0: f64, proc: f64, ok: bool, nl: usize, fnlen: usize }
/// KT128 of S (already in the buffer at offset 0, already visible to the atoms) on atoms 0..used
unsafe fn kt_atoms(engs: &[Eng], used: usize, b: &Big, slen: usize, olen: usize) -> (Vec<u8>, KtT) {
    let t = &ALGS[T128]; let fnv = b.va.add(KT_FO); let fnp = b.pa + KT_FO as u64; let outp = b.pa + KT_OO as u64;
    let mut r = KtT { run: 0.0, leaves: 0.0, fin: 0.0, s0: 0.0, proc: 0.0, ok: true, nl: 0, fnlen: slen };
    let t0 = now_ns(); let nch = (slen + KTC - 1) / KTC;
    if nch <= 1 { go(&engs[0], t, 0x07, slen, olen, b.pa, outp, 1); let tg = now_ns(); r.proc += tg - t0; r.ok &= wait1(&engs[0]); r.fin = now_ns() - tg; }
    else {
        let nl = nch - 1; let last = slen - nl * KTC; let nfull = if last == KTC { nl } else { nl - 1 }; r.nl = nl;
        // final node = S_0 || 03 00^7 || CV_1 .. CV_nl || length_encode(nl) || FF FF. Header and trailer first, written back and dropped from the
        // cache before the atoms write the CVs between them (they share cache lines with the CVs).
        let hdr = [3u8, 0, 0, 0, 0, 0, 0, 0]; std::ptr::copy_nonoverlapping(hdr.as_ptr(), fnv.add(KTC), 8);
        let mut tr = lenc(nl); tr.push(0xFF); tr.push(0xFF); let cve = KTC + 8 + 32 * nl;
        std::ptr::copy_nonoverlapping(tr.as_ptr(), fnv.add(cve), tr.len()); std::ptr::write_bytes(fnv.add(cve + tr.len()), 0, 64);
        let fnlen = cve + tr.len(); r.fnlen = fnlen; from_device(fnv.add(KTC), fnlen + 64 - KTC);
        // full leaves: contiguous ranges, one batch per atom, read in place from S, CVs written in place into the final node
        let per = nfull / used; let rem = nfull % used; let mut s = 1usize; let mut busy = vec![false; used];
        for a in 0..used { let c = per + (a < rem) as usize; if c == 0 { continue; }
            go(&engs[a], t, 0x0B, KTC, 32, b.pa + (s * KTC) as u64, fnp + (KTC + 8 + 32 * (s - 1)) as u64, c); busy[a] = true; s += c; }
        let mut part = last != KTC;
        // while the atoms hash the leaves: S_0 into the final node (the only copy in KT128 on the atoms)
        let ts = now_ns(); std::ptr::copy_nonoverlapping(b.va, fnv, KTC); to_device(fnv, KTC); let tse = now_ns(); r.s0 = tse - ts; r.proc += tse - t0;
        let mut spins = 0u64;
        loop { let mut any = false;
            for a in 0..used { if busy[a] { if let Some(st) = engs[a].finished() { r.ok &= st & 0x72 == 2; busy[a] = false; } else { any = true; } }
                if !busy[a] && part { let tp = now_ns(); go(&engs[a], t, 0x0B, last, 32, b.pa + (nl * KTC) as u64, fnp + (KTC + 8 + 32 * (nl - 1)) as u64, 1);
                    busy[a] = true; part = false; any = true; r.proc += now_ns() - tp; } }
            if !any { break; } spins += 1; if spins > 3_000_000_000 { r.ok = false; break; } }
        let tl = now_ns(); r.leaves = tl - t0;
        go(&engs[0], t, 0x06, fnlen, olen, fnp, outp, 1); let tg = now_ns(); r.proc += tg - tl; r.ok &= wait1(&engs[0]); r.fin = now_ns() - tl; }
    let tq = now_ns(); let op = b.va.add(KT_OO); from_device(op, olen); let d = std::slice::from_raw_parts(op, olen).to_vec(); let t2 = now_ns();
    r.proc += t2 - tq; r.run = t2 - t0; (d, r) }
/// flat XOF of the first len bytes of the buffer on one atom, in place
unsafe fn flat_atom(e: &Eng, g: &Alg, b: &Big, len: usize, olen: usize) -> (Vec<u8>, f64, bool) {
    let t0 = now_ns(); go(e, g, 0x1F, len, olen, b.pa, b.pa + KT_OO as u64, 1); let ok = wait1(e); let op = b.va.add(KT_OO); from_device(op, olen);
    let d = std::slice::from_raw_parts(op, olen).to_vec(); (d, now_ns() - t0, ok) }

unsafe fn kt_main(engs: &[Eng], n: usize, mhz: f64, reps: usize, b: &Big) -> usize {
    let mut bad = 0usize; let counts: Vec<usize> = [1usize, 2, 4].iter().cloned().filter(|&u| u <= n).collect();
    // ---- (1) the 18 KT128 test vectors of RFC 9861 Section 5 (full expected values as in XKCP / pycryptodome; first 8 bytes checked against the RFC) ----
    let ff = |k: usize| vec![0xFFu8; k];
    let vecs: Vec<(String, Vec<u8>, Vec<u8>, usize, &str)> = vec![
        ("M=empty,C=empty,32".to_string(), vec![], vec![], 32, "1AC2D450FC3B4205D19DA7BFCA1B37513C0803577AC7167F06FE2CE1F0EF39E5"),
        ("M=empty,C=empty,64".to_string(), vec![], vec![], 64, "1AC2D450FC3B4205D19DA7BFCA1B37513C0803577AC7167F06FE2CE1F0EF39E54269C056B8C82E48276038B6D292966CC07A3D4645272E31FF38508139EB0A71"),
        ("M=empty,C=empty,10032(last32)".to_string(), vec![], vec![], 10032, "E8DC563642F7228C84684C898405D3A834799158C079B12880277A1D28E2FF6D"),
        ("M=ptn(1)".to_string(), ptn(1), vec![], 32, "2BDA92450E8B147F8A7CB629E784A058EFCA7CF7D8218E02D345DFAA65244A1F"),
        ("M=ptn(17)".to_string(), ptn(17), vec![], 32, "6BF75FA2239198DB4772E36478F8E19B0F371205F6A9A93A273F51DF37122888"),
        ("M=ptn(17^2)".to_string(), ptn(289), vec![], 32, "0C315EBCDEDBF61426DE7DCF8FB725D1E74675D7F5327A5067F367B108ECB67C"),
        ("M=ptn(17^3)".to_string(), ptn(4913), vec![], 32, "CB552E2EC77D9910701D578B457DDF772C12E322E4EE7FE417F92C758F0D59D0"),
        ("M=ptn(17^4)".to_string(), ptn(83521), vec![], 32, "8701045E22205345FF4DDA05555CBB5C3AF1A771C2B89BAEF37DB43D9998B9FE"),
        ("M=ptn(17^5)".to_string(), ptn(1419857), vec![], 32, "844D610933B1B9963CBDEB5AE3B6B05CC7CBD67CEEDF883EB678A0A8E0371682"),
        ("M=ptn(17^6)".to_string(), ptn(24137569), vec![], 32, "3C390782A8A4E89FA6367F72FEAAF13255C8D95878481D3CD8CE85F58E880AF8"),
        ("M=empty,C=ptn(1)".to_string(), vec![], ptn(1), 32, "FAB658DB63E94A246188BF7AF69A133045F46EE984C56E3C3328CAAF1AA1A583"),
        ("M=FF,C=ptn(41)".to_string(), ff(1), ptn(41), 32, "D848C5068CED736F4462159B9867FD4C20B808ACC3D5BC48E0B06BA0A3762EC4"),
        ("M=FF^3,C=ptn(41^2)".to_string(), ff(3), ptn(1681), 32, "C389E5009AE57120854C2E8C64670AC01358CF4C1BAF89447A724234DC7CED74"),
        ("M=FF^7,C=ptn(41^3)".to_string(), ff(7), ptn(68921), 32, "75D2F86A2E644566726B4FBCFC5657B9DBCF070C7B0DCA06450AB291D7443BCF"),
        ("M=ptn(8191)".to_string(), ptn(8191), vec![], 32, "1B577636F723643E990CC7D6A659837436FD6A103626600EB8301CD1DBE553D6"),
        ("M=ptn(8192)".to_string(), ptn(8192), vec![], 32, "48F256F6772F9EDFB6A8B661EC92DC93B95EBD05A08A17B39AE3490870C926C3"),
        ("M=ptn(8192),C=ptn(8189)".to_string(), ptn(8192), ptn(8189), 32, "3ED12F70FB05DDB58689510AB3E4D23C6C6033849AA01E1D8C220A297FEDCD0B"),
        ("M=ptn(8192),C=ptn(8190)".to_string(), ptn(8192), ptn(8190), 32, "6A7C1B6A5CD0D8C9CA943A4A216CC64604559A2EA45F78570A15253D67BA00AE")];
    let mut vok = 0usize; let mut vtot = 0usize;
    for (vi, (name, m, c, olen, want)) in vecs.iter().enumerate() { let s = kt_s(m, c); let want = unhx(want);
        let sw = kt128_sw(&s, *olen); let swg = if *olen == 10032 { sw[10000..].to_vec() } else { sw.clone() }; let swok = swg == want;
        put_s(b, &s); to_device(b.va, s.len());
        for &u in counts.iter() { let (d, r) = kt_atoms(engs, u, b, s.len(), *olen); let dg = if *olen == 10032 { d[10000..].to_vec() } else { d.clone() };
            let ok = r.ok && dg == want; vtot += 1; if ok { vok += 1; } else { bad += 1; }
            println!("RESULT ktvec idx={} name={} slen={} leaves={} atoms={} sw_ok={} atoms_ok={} got={}", vi + 1, name, s.len(), r.nl, u, swok, ok, hx(&dg[..16])); }
        if !swok { bad += 1; } }
    println!("RESULT ktvec summary vectors={} atom_runs={} atom_runs_ok={} {}", vecs.len(), vtot, vok, if vok == vtot { "ALL_RFC9861_VECTORS_MATCH" } else { "FAIL" });
    // ---- (2) edge lengths, random content, against the program's software KT128 ----
    let mut rng = Rng(0x0BAD_5EED_1234_0001); let mut eok = 0usize; let mut etot = 0usize;
    for &(ml, cl) in [(0usize, 0usize), (8190, 0), (8191, 0), (8192, 0), (8193, 0), (16382, 0), (16383, 0), (16384, 0), (16385, 0), (8192 * 5 + 100, 4), (8192 * 64 - 1, 0),
                      (300_000, 7), (8192 * 9 - 1, 0), (1 << 20, 0), ((1 << 20) + 4097, 3)].iter() {
        let m = rng.msg(ml); let c = rng.msg(cl); let s = kt_s(&m, &c); let want = kt128_sw(&s, 32); put_s(b, &s); to_device(b.va, s.len());
        for &u in counts.iter() { let (d, r) = kt_atoms(engs, u, b, s.len(), 32); let ok = r.ok && d == want; etot += 1; if ok { eok += 1; } else { bad += 1;
            println!("RESULT ktedge FAIL mlen={} clen={} atoms={} got={} want={}", ml, cl, u, hx(&d), hx(&want)); } } }
    println!("RESULT ktedge summary runs={} ok={} {}", etot, eok, if eok == etot { "ALL_MATCH" } else { "FAIL" });
    // ---- (3) timed: KT128 versus flat SHAKE128 / TurboSHAKE128 of the same message (one stream, prefixes), C empty ----
    let stream = Rng(0x4B54_3132_5F4D_5347).msg(16 << 20);
    for &ml in [65536usize, 1 << 20, 16 << 20].iter() {
        let m = &stream[..ml]; let s = kt_s(m, &[]); let want = kt128_sw(&s, 32); put_s(b, &s);
        // processor cost of making a CPU-written message visible to the atoms (clean every line), 5 times on freshly written data
        let mut fl = Vec::new(); for _ in 0..5 { put_s(b, &s); let t0 = now_ns(); to_device(b.va, s.len()); fl.push(now_ns() - t0); }
        let flen = if ml >= (1 << 24) { ml - 8 } else { ml };          // flat jobs: the LEN field is 24 bits, so 16 MB flat is 16 MB - 8 bytes
        let wsh = xofd(&ALGS[S128], 0x1F, &m[..flen], 32); let wts = xofd(&ALGS[T128], 0x1F, &m[..flen], 32);
        println!("RESULT ktdig mlen={} clen=0 olen=32 kt128={} flat_len={} shake128={} turboshake128={} clean_message_us_median={:.2}", ml, hx(&want), flen, hx(&wsh), hx(&wts), med(&fl) / 1e3);
        for &u in counts.iter() {
            let mut tr = Vec::new(); let mut tl = Vec::new(); let mut tf = Vec::new(); let mut tp = Vec::new(); let mut t0c = Vec::new(); let mut mis = 0usize; let mut last = KtT { run: 0.0, leaves: 0.0, fin: 0.0, s0: 0.0, proc: 0.0, ok: true, nl: 0, fnlen: 0 };
            for i in 0..reps + 2 { let (d, r) = kt_atoms(engs, u, b, s.len(), 32); if !(r.ok && d == want) { mis += 1; continue; }
                if i >= 2 { tr.push(r.run); tl.push(r.leaves); tf.push(r.fin); tp.push(r.proc); t0c.push(r.s0); } last = r; }
            bad += mis; if tr.is_empty() { println!("RESULT kt FAILED mlen={} atoms={} mismatches={}", ml, u, mis); continue; }
            let (mt, st) = stats(&tr); let moved = (s.len() + last.fnlen + 32 * last.nl + 32) as f64;
            println!("RESULT kt mlen={} atoms={} engine_mhz={} leaves={} final_node_bytes={} {} Gbps_median={:.3} port_share={:.3} leaf_phase_us={:.2} final_node_us={:.2} s0_copy_us={:.2} proc_us={:.2} proc_share={:.3} e2e_with_clean_us={:.2} mismatches={} {}",
                ml, u, mhz, last.nl, last.fnlen, st, ml as f64 * 8.0 / mt, moved / (2.4 * mt), med(&tl) / 1e3, med(&tf) / 1e3, med(&t0c) / 1e3, med(&tp) / 1e3, med(&tp) / mt, (mt + med(&fl)) / 1e3, mis, if mis == 0 { "VERIFIED" } else { "FAILED" }); }
        for &(gi, want) in [(S128, &wsh), (T128, &wts)].iter() { let g = &ALGS[gi];
            let mut tr = Vec::new(); let mut mis = 0usize;
            for i in 0..reps + 2 { let (d, t, ok) = flat_atom(&engs[0], g, b, flen, 32); if !(ok && &d == want) { mis += 1; continue; } if i >= 2 { tr.push(t); } }
            bad += mis; if tr.is_empty() { println!("RESULT flat FAILED alg={} mlen={} mismatches={}", g.name, flen, mis); continue; }
            let (mt, st) = stats(&tr);
            println!("RESULT flat alg={} mlen={} atoms_in_image={} atoms_used=1 engine_mhz={} {} Gbps_median={:.3} port_share={:.3} e2e_with_clean_us={:.2} mismatches={} {}",
                g.name, flen, n, mhz, st, flen as f64 * 8.0 / mt, (flen + 32) as f64 / (2.4 * mt), (mt + med(&fl)) / 1e3, mis, if mis == 0 { "VERIFIED" } else { "FAILED" }); } }
    bad }

// ======================= Merkle tree over 2^k 32-byte values =======================
fn mode_alg(mi: usize) -> (Alg, u8, &'static str) { match mi { 0 => (ALGS[M128], 0x01, "minishake128_split_NONSTANDARD_TREE"), 1 => (ALGS[T128], 0x1F, "turboshake128_wide_NONSTANDARD_TREE"), _ => (ALGS[S128], 0x1F, "shake128_wide") } }
fn merkle_sw(vals: &[u8], g: &Alg, dom: u8, k: u32) -> Vec<u8> { let n = 1usize << k;
    let mut lv: Vec<u8> = Vec::with_capacity(n * 32); for i in 0..n { lv.extend_from_slice(&xofd(g, dom, &vals[i * 32..i * 32 + 32], 32)); }
    let mut m = n; while m > 1 { let mut nx = Vec::with_capacity(m * 16); for i in 0..m / 2 { nx.extend_from_slice(&xofd(g, dom, &lv[i * 64..i * 64 + 64], 32)); } lv = nx; m /= 2; }
    lv }
/// split layout: atom a hashes subtrees 2a (core 0) and 2a+1 (core 1) of the 2*used equal subtrees in lockstep; pair j word w = 4 bytes of each
unsafe fn pack_split(vals: &[u8], k: u32, used: usize, dst: *mut u8) { let m = (1usize << k) / (2 * used);
    for a in 0..used { let ra = a * m * 64; let xs = 2 * a * m; let ys = (2 * a + 1) * m;
        for j in 0..m { for w in 0..8 { let d = dst.add(ra + j * 64 + w * 8);
            std::ptr::copy_nonoverlapping(vals.as_ptr().add((xs + j) * 32 + 4 * w), d, 4); std::ptr::copy_nonoverlapping(vals.as_ptr().add((ys + j) * 32 + 4 * w), d.add(4), 4); } } } }
struct MT { run: f64, proc: f64, top: f64, batches: usize, pads: usize, ok: bool }
/// values already laid out in V (offset 0) and visible to the atoms; work area W at 32*2^k; top scratch after W
unsafe fn merkle_atoms(engs: &[Eng], used: usize, mi: usize, k: u32, b: &Big) -> (Vec<u8>, MT) {
    let (g, dom, _) = mode_alg(mi); let dual = g.w == 32; let nleaf = 1usize << k; let wo = 32usize << k; let to = wo + (32usize << k);
    let units = if dual { nleaf / (2 * used) } else { nleaf / used }; let reg = nleaf * 32 / used;
    let mut r = MT { run: 0.0, proc: 0.0, top: 0.0, batches: 0, pads: 0, ok: true };
    let t0 = now_ns(); let mut cnt = vec![units; used]; let mut busy = vec![true; used];
    for a in 0..used { go(&engs[a], &g, dom, 32, 32, b.pa + (a * reg) as u64, b.pa + (wo + a * reg) as u64, cnt[a]); r.batches += 1; }
    r.proc += now_ns() - t0; let mut left = used; let mut spins = 0u64;
    while left > 0 { for a in 0..used { if busy[a] { if let Some(st) = engs[a].finished() { let tp = now_ns(); r.ok &= st & 0x72 == 2;
                if cnt[a] == 1 { busy[a] = false; left -= 1; } else { cnt[a] /= 2; let p = b.pa + (wo + a * reg) as u64; go(&engs[a], &g, dom, 64, 32, p, p, cnt[a]); r.batches += 1; }
                r.proc += now_ns() - tp; } } }
        spins += 1; if spins > 3_000_000_000 { r.ok = false; break; } }
    // subtree roots -> the top log2(roots) levels on atom 0 (laid out by the processor; at most 8 roots)
    let tt = now_ns(); let mut roots: Vec<Vec<u8>> = Vec::new();
    for a in 0..used { let p = b.va.add(wo + a * reg); from_device(p, 64);
        if dual { for h in 0..2 { roots.push((0..32).map(|i| *p.add((i / 4) * 8 + h * 4 + i % 4)).collect()); } } else { roots.push(std::slice::from_raw_parts(p, 32).to_vec()); } }
    let tv = b.va.add(to); let tpa = b.pa + to as u64;
    while roots.len() > 1 { let nn = roots.len() / 2; let tl = now_ns();
        let msgs: Vec<Vec<u8>> = (0..nn).map(|i| { let mut v = roots[2 * i].clone(); v.extend_from_slice(&roots[2 * i + 1]); v }).collect();
        let (nj, ib, ob) = if dual { let pairs = (nn + 1) / 2; r.pads += 2 * pairs - nn;
                for q in 0..2 * pairs { let mm = &msgs[if q < nn { q } else { nn - 1 }]; for i in 0..64 { *tv.add((q / 2) * 128 + (i / 4) * 8 + (q % 2) * 4 + i % 4) = mm[i]; } } (pairs, pairs * 128, pairs * 64) }
            else { for q in 0..nn { std::ptr::copy_nonoverlapping(msgs[q].as_ptr(), tv.add(q * 64), 64); } (nn, nn * 64, nn * 32) };
        to_device(tv, ib); go(&engs[0], &g, dom, 64, 32, tpa, tpa + 4096, nj); r.batches += 1; let tw = now_ns(); r.proc += tw - tl; r.ok &= wait1(&engs[0]);
        let tu = now_ns(); from_device(tv.add(4096), ob); let ov = tv.add(4096);
        roots = (0..nn).map(|q| if dual { (0..32).map(|i| *ov.add((q / 2) * 64 + (i / 4) * 8 + (q % 2) * 4 + i % 4)).collect() } else { std::slice::from_raw_parts(ov.add(q * 32), 32).to_vec() }).collect();
        r.proc += now_ns() - tu; }
    let te = now_ns(); r.top = te - tt; r.run = te - t0; (roots[0].clone(), r) }

unsafe fn merkle_main(engs: &[Eng], n: usize, mhz: f64, reps: usize, hf: c_int, pm: &mut File, ks: &[u32]) -> usize {
    let mut bad = 0usize; let counts: Vec<usize> = [1usize, 2, 4].iter().cloned().filter(|&u| u <= n).collect();
    for &k in ks.iter() { let nleaf = 1usize << k; let cap = 64 * nleaf + 8192;
        let b = match big(hf, pm, cap) { Some(x) => x, None => { println!("RESULT merkle k={} error=no_contiguous_buffer bytes={}", k, cap); bad += 1; continue; } };
        for e in engs.iter() { win(e, b.pa, b.cap); }
        let vals = Rng(0x4D45_524B_4C45_0000 + k as u64).msg(32 * nleaf); let hashes = 2 * nleaf - 1;
        println!("RESULT merkle_values k={} leaves={} rng_seed={:#x} first16={}", k, nleaf, 0x4D45_524B_4C45_0000u64 + k as u64, hx(&vals[..16]));
        for mi in 0..3 { let (g, dom, label) = mode_alg(mi); let want = merkle_sw(&vals, &g, dom, k);
            println!("RESULT merkle_root k={} mode={} alg={} dom={:#04x} sw_root={}", k, label, g.name, dom, hx(&want));
            for &u in counts.iter() {
                // lay the values out in V (wide: as they are; split: interleaved per atom) and make them visible to the atoms
                let mut pk = Vec::new(); let mut fl = Vec::new();
                for _ in 0..3 { let t0 = now_ns(); if g.w == 32 { pack_split(&vals, k, u, b.va); } else { std::ptr::copy_nonoverlapping(vals.as_ptr(), b.va, 32 * nleaf); }
                    let t1 = now_ns(); to_device(b.va, 32 * nleaf); let t2 = now_ns(); pk.push(t1 - t0); fl.push(t2 - t1); }
                let mut tr = Vec::new(); let mut tp = Vec::new(); let mut tt = Vec::new(); let mut mis = 0usize; let mut nb = 0usize; let mut pads = 0usize; let mut got = Vec::new();
                for i in 0..reps + 2 { let (root, r) = merkle_atoms(engs, u, mi, k, &b); if !(r.ok && root == want) { mis += 1; got = root; continue; }
                    if i >= 2 { tr.push(r.run); tp.push(r.proc); tt.push(r.top); } nb = r.batches; pads = r.pads; }
                bad += mis; if tr.is_empty() { println!("RESULT merkle FAILED k={} mode={} atoms={} mismatches={} got={}", k, label, u, mis, hx(&got)); continue; }
                let (mt, st) = stats(&tr); let hps = hashes as f64 / (mt / 1e9); let pkm = if g.w == 32 { med(&pk) } else { 0.0 };
                println!("RESULT merkle k={} mode={} atoms={} engine_mhz={} hashes={} batches={} pad_hashes={} {} Mhash_per_s={:.3} Mhash_per_s_per_atom={:.3} proc_us={:.2} proc_share={:.3} top_us={:.2} layout_pack_us={:.2} clean_values_us={:.2} e2e_with_layout_us={:.2} proc_share_e2e={:.3} mismatches={} {}",
                    k, label, u, mhz, hashes, nb, pads, st, hps / 1e6, hps / 1e6 / u as f64, med(&tp) / 1e3, med(&tp) / mt, med(&tt) / 1e3, pkm / 1e3, med(&fl) / 1e3,
                    (mt + pkm + med(&fl)) / 1e3, (med(&tp) + pkm + med(&fl)) / (mt + pkm + med(&fl)), mis, if mis == 0 { "VERIFIED" } else { "FAILED" }); } } }
    bad }

fn main() { unsafe {
    for r in 0..24 { RCT[r] = rc(r as u32); }
    let args: Vec<String> = std::env::args().collect(); let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mhz: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0); let what = args.get(3).cloned().unwrap_or("kt".to_string());
    let reps: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(25);
    let ks: Vec<u32> = args.get(5).cloned().unwrap_or("16".to_string()).split(',').filter_map(|s| s.parse().ok()).collect();
    let (engs, hf, mut pm) = match setup(n) { Some(x) => x, None => return };
    println!("RESULT modes engines={} engine_mhz={} what={} reps={} ids={} arch={}", n, mhz, what, reps, engs.iter().map(|e| format!("{:08x}", e.rd(ID))).collect::<Vec<String>>().join(" "),
        if cfg!(target_arch = "aarch64") { "board" } else { "EMULATOR" });
    let bad = if what == "kt" { let b = match big(hf, &mut pm, KT_CAP) { Some(x) => x, None => { println!("RESULT error=no_contiguous_buffer bytes={}", KT_CAP); return; } };
            for e in engs.iter() { win(e, b.pa, b.cap); } kt_main(&engs, n, mhz, reps, &b) }
        else { merkle_main(&engs, n, mhz, reps, hf, &mut pm, &ks) };
    println!("RESULT summary what={} mismatches={} {}", what, bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
