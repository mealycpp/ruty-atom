// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_sign5.rs (NEW, 2026-11-26): ruty_sign4.rs with one change, so that every median can be recomputed from the individual runs:
// fn stats() also prints samples_ms=<the n times, in run order>. Nothing else differs from ruty_sign4.rs.
// ruty_sign: a COMPLETE SLH-DSA-SHAKE-128f (FIPS 205) key generation, signature and verification computed on the RUTY atoms,
// driven by a job-graph scheduler on one Cortex-A53 core (scheduler and driver taken over from ruty_dfg2.rs; this file is new).
//   gcc -c cacheops.S -o cacheops.o && rustc -O -C link-arg=cacheops.o -o ruty_sign ruty_sign.rs
//   sudo ./ruty_sign <atoms in the image> <engine MHz> <std|fw|turbo|mini> [repetitions]
// Modes: std   = SLH-DSA-SHAKE-128f exactly as FIPS 205 (pure, empty context, deterministic: opt_rand = PK.seed).
//        fw    = firmware authentication: SHAKE256 of a 1 MB image on an atom (no-copy) + HashSLH-DSA-SHAKE-128f (FIPS 205,
//                pre-hash SHAKE256 with 512-bit output, OID 2.16.840.1.101.3.4.2.12) signed and verified on the atoms.
//        turbo = NON-STANDARD: the same signature structure with TurboSHAKE256 (12 rounds) in every call. NOT FIPS 205.
//        mini  = NON-STANDARD: the same structure with MiniSHAKE128 (Keccak-p[800], 12 rounds, ePrint 2026/2363, a preprint) in every
//                call, on split atoms (two Keccak-p[800] per atom). NOT FIPS 205, no external reference exists.
// Built for the board (aarch64). Built anywhere else, the atoms are replaced by an emulator of atom_seq3.v (for checking the
// scheduler and the data layout against an independent implementation before any board run).
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
const PAGE: usize = 2 << 20; const HALF: usize = PAGE / 2;
fn now_ns() -> f64 { let mut t = Timespec { tv_sec: 0, tv_nsec: 0 }; unsafe { clock_gettime(1, &mut t); } t.tv_sec as f64 * 1e9 + t.tv_nsec as f64 }

// ---------------- software Keccak-p with 64-bit or 32-bit lanes (from ruty_dfg2.rs) ----------------
const RHO: [u32; 25] = [0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14];
fn rc(r: u32) -> u64 { let mut l: u32 = 1; let mut v = 0u64;
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
fn hx(b: &[u8]) -> String { let mut s = String::with_capacity(b.len() * 2); for x in b { s.push_str(&format!("{:02x}", x)); } s }

// ---------------- one atom (from ruty_dfg2.rs) ----------------
struct Eng { reg: *mut u32, va: *mut u8, pa: u64 }
impl Eng {
    unsafe fn wr(&self, off: usize, v: u32) { write_volatile(self.reg.add(off / 4), v) }
    unsafe fn rd(&self, off: usize) -> u32 { read_volatile(self.reg.add(off / 4)) }
    fn buf(&self, half: usize) -> (*mut u8, u64) { unsafe { (self.va.add(half * HALF), self.pa + (half * HALF) as u64) } }
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
fn adrs(layer: u32, tree: u64, typ: u32, kp: u32, a: u32, b: u32) -> [u8; 32] { let mut o = [0u8; 32];
    o[0..4].copy_from_slice(&layer.to_be_bytes()); o[8..16].copy_from_slice(&tree.to_be_bytes()); o[16..20].copy_from_slice(&typ.to_be_bytes());
    o[20..24].copy_from_slice(&kp.to_be_bytes()); o[24..28].copy_from_slice(&a.to_be_bytes()); o[28..32].copy_from_slice(&b.to_be_bytes()); o }

// ---------------- emulator of atom_seq3.v (only when NOT built for the board) ----------------
#[cfg(not(target_arch = "aarch64"))]
unsafe fn emu(e: &Eng) {
    let lr = e.rd(LEN); let len = (lr & 0xFF_FFFF) as usize; let dom = (lr >> 24) as u8; let olen = e.rd(OLEN) as usize; let alg = e.rd(ALG);
    let (first, last, dual, rl) = (alg & 31, (alg >> 8) & 31, (alg >> 16) & 1 == 1, ((alg >> 24) & 31) as usize);
    let ctl = e.rd(0x44); let iter = ctl & 1 == 1; let cw = ((ctl >> 8) & 31) as usize; let cb = ((ctl >> 16) & 7) as usize; let fbw = ((ctl >> 20) & 31) as usize;
    let cend = (e.rd(0x48) & 255) as u8; let nj = e.rd(NJOBS) as usize;
    let inp = (e.rd(IN_LO) as u64 | ((e.rd(IN_HI) as u64) << 32)) as usize as *const u8; let out = (e.rd(OUT_LO) as u64 | ((e.rd(OUT_HI) as u64) << 32)) as usize as *mut u8;
    let nb = if dual { 4 } else { 8 }; let wpm = (len + nb - 1) / nb; let wpo = (olen + nb - 1) / nb; let w = if dual { 32 } else { 64 };
    let g = Alg { name: "emu", w: w, rate: rl * nb, first: first, last: last }; let nc = if dual { 2 } else { 1 };
    for j in 0..nj { let src = inp.add(j * wpm * 8); let dst = out.add(j * wpo * 8);
        if !iter {
            for h in 0..nc { let msg: Vec<u8> = (0..len).map(|i| if dual { *src.add((i / 4) * 8 + h * 4 + i % 4) } else { *src.add(i) }).collect();
                let d = xofd(&g, dom, &msg, olen);
                for i in 0..olen { if dual { *dst.add((i / 4) * 8 + h * 4 + i % 4) = d[i]; } else { *dst.add(i) = d[i]; } } }
        } else {
            assert!(len < g.rate, "chain message must be one block");
            let mut t = [0u64; 25];
            for h in 0..nc { let mut s = [0u8; 200]; for i in 0..len { s[i] = if dual { *src.add((i / 4) * 8 + h * 4 + i % 4) } else { *src.add(i) }; }
                s[len] ^= dom; s[g.rate - 1] ^= 0x80;
                for i in 0..25 { let mut v = 0u64; for k in 0..nb { v |= (s[nb * i + k] as u64) << (8 * k); } t[i] |= if dual { v << (32 * h) } else { v }; } }
            let tc = t[cw]; let (b0, b1) = if dual { (cb & 3, 4 + (cb & 3)) } else { (cb, 0) };
            let mut c0 = (tc >> (8 * b0)) as u8; let mut c1 = (tc >> (8 * b1)) as u8;
            let setb = |d: u64, b: usize, v: u8| (d & !(0xffu64 << (8 * b))) | ((v as u64) << (8 * b));
            let nfb = wpo & 7; let mut a = t;
            loop {
                t[cw] = if dual { setb(setb(tc, b0, c0.wrapping_add(1)), b1, c1.wrapping_add(1)) } else { setb(tc, b0, c0.wrapping_add(1)) };
                if dual { let mut lo = [0u64; 25]; let mut hi = [0u64; 25]; for i in 0..25 { lo[i] = a[i] & 0xffff_ffff; hi[i] = a[i] >> 32; }
                    keccak_p(&mut lo, 32, first, last); keccak_p(&mut hi, 32, first, last); for i in 0..25 { a[i] = lo[i] | (hi[i] << 32); } }
                else { keccak_p(&mut a, 64, first, last); }
                if c0.wrapping_add(1) == cend { break; }
                for i in 0..nfb { t[fbw + i] = a[i]; } c0 = c0.wrapping_add(1); c1 = c1.wrapping_add(1); a = t; }
            for i in 0..wpo { let v = a[i]; for k in 0..8 { *dst.add(i * 8 + k) = (v >> (8 * k)) as u8; } } } }
    e.wr(STATUS, 2); }

// ruty_sign4 (NEW, from ruty_sign3.rs): firmware authentication in every mode (fw = FIPS 205 HashSLH-DSA with SHAKE256; fw-turbo, fw-mini,
// fw-hybrid = NON-STANDARD: pre-hash TurboSHAKE256-512 of the image on a wide atom, signature in that mode, no OID exists, the OID field is
// 11 zero bytes), mode tenants (4-atom image: SLH-DSA sign and firmware verify at once on disjoint atoms, two A53 cores), and split-atom
// verify chains in one batch per layer (pairs with equal start digit; a chain without a partner is paired with a copy of itself).
//   sudo ./ruty_sign4 <atoms> <engine MHz> <std|turbo|mini|hybrid|fw|fw-turbo|fw-mini|fw-hybrid|tenants> [repetitions] [old|word]
// ruty_sign3 (NEW, from ruty_sign.rs): adds the hybrid suite, a word-wise split-atom layout (arg 5 "old" restores the ruty_sign.rs layout),
// two XMSS trees per node in the pure split mode, a per-routine processor profile, and verify on fewer atoms.
//   sudo ./ruty_sign3 <atoms> <engine MHz> <std|fw|turbo|mini|hybrid> [repetitions] [old|word]

// ================= SLH-DSA-SHAKE-128f parameters (FIPS 205, Table 2) =================
const N: usize = 16; const WL: usize = 35; const HP: usize = 3; const D: usize = 22; const FA: usize = 6; const FK: usize = 33; const FT: usize = 64;
const SIGLEN: usize = N * (1 + FK * (FA + 1) + D * (WL + HP));          // 17,088 bytes
const HTOFF: usize = N * (1 + FK * (FA + 1));                             // offset of SIG_HT
const NCT: usize = 8 * WL;                                                 // 280 chains per XMSS tree
// ADRS types (FIPS 205, Table 1)
const WOTS_HASH: u32 = 0; const WOTS_PK: u32 = 1; const TREE: u32 = 2; const FORS_TREE: u32 = 3; const FORS_ROOTS: u32 = 4; const WOTS_PRF: u32 = 5; const FORS_PRF: u32 = 6;
const OID_SHAKE256: [u8; 11] = [0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x0C];

// The hash used for every call (PRF, PRF_msg, H_msg, F, H, T_l). Standard: SHAKE256. The other two are NOT FIPS 205.
#[derive(Clone, Copy)] struct Hf { name: &'static str, g: Alg, dom: u8, dual: bool }
fn hf_of(mode: &str) -> Hf { match mode {
    "turbo" => Hf { name: "turboshake256", g: ALGS[3], dom: 0x1F, dual: false },
    "mini" => Hf { name: "minishake128", g: ALGS[4], dom: 0x01, dual: true },
    _ => Hf { name: "shake256", g: ALGS[1], dom: 0x1F, dual: false } } }
// The pair of hashes used: f for the one-block calls (PRF and F: 64-byte inputs), h for the rest (H, T_len, T_k, H_msg, PRF_msg).
// std: both SHAKE256 (FIPS 205). turbo, mini: both the same non-standard function. hybrid (NON-STANDARD, our choice): f = MiniSHAKE128 on
// split atoms, h = TurboSHAKE256 on wide atoms; each atom is set to the width of the call it runs, per batch.
#[derive(Clone, Copy)] struct Suite { name: &'static str, f: Hf, h: Hf }
fn suite_of(mode: &str) -> Suite { match mode {
    "turbo" => Suite { name: "turboshake256", f: hf_of("turbo"), h: hf_of("turbo") },
    "mini" => Suite { name: "minishake128", f: hf_of("mini"), h: hf_of("mini") },
    "hybrid" => Suite { name: "hybrid(minishake128:PRF,F|turboshake256:H,T_l,H_msg,PRF_msg)", f: hf_of("mini"), h: hf_of("turbo") },
    _ => Suite { name: "shake256", f: hf_of("std"), h: hf_of("std") } } }

// ---------------- shared helpers (FIPS 205 Algorithms 4, 5 and the digest split of Algorithm 19) ----------------
fn base16(m: &[u8]) -> [u8; WL] { let mut d = [0u8; WL]; let mut cs = 0u32;
    for i in 0..32 { d[i] = if i % 2 == 0 { m[i / 2] >> 4 } else { m[i / 2] & 15 }; cs += 15 - d[i] as u32; }
    cs <<= 4;                                                     // (8 - ((len2 * lg_w) mod 8)) mod 8 = 4
    let cb = [(cs >> 8) as u8, cs as u8]; d[32] = cb[0] >> 4; d[33] = cb[0] & 15; d[34] = cb[1] >> 4; d }
fn fors_idx(md: &[u8]) -> [u32; FK] { let mut o = [0u32; FK]; let (mut i, mut bits, mut tot) = (0usize, 0u32, 0u64);
    for k in 0..FK { while bits < FA as u32 { tot = (tot << 8) | md[i] as u64; i += 1; bits += 8; } bits -= FA as u32; o[k] = ((tot >> bits) & 63) as u32; tot &= (1u64 << bits) - 1; } o }
struct Idx { fidx: [u32; FK], itree: u64, ileaf: u32, ltree: [u64; D], lleaf: [u32; D] }
fn split_digest(dg: &[u8]) -> Idx { let fidx = fors_idx(&dg[0..25]);
    let mut t = 0u64; for b in &dg[25..33] { t = (t << 8) | *b as u64; } let itree = t & ((1u64 << 63) - 1); let ileaf = (dg[33] & 7) as u32;
    let mut ltree = [0u64; D]; let mut lleaf = [0u32; D];
    for j in 0..D { ltree[j] = if j == 0 { itree } else { itree >> (3 * j) }; lleaf[j] = if j == 0 { ileaf } else { ((itree >> (3 * (j - 1))) & 7) as u32 }; }
    Idx { fidx: fidx, itree: itree, ileaf: ileaf, ltree: ltree, lleaf: lleaf } }

// ================= software SLH-DSA on the A53 (the program's own reference and its software verify; not independent) =================
fn hs(hf: &Hf, parts: &[&[u8]], olen: usize) -> Vec<u8> { xofd(&hf.g, hf.dom, &cat(parts), olen) }
fn sw_chain(su: &Suite, seed: &[u8], x: &[u8], layer: u32, tree: u64, kp: u32, c: u32, s0: u32, steps: u32) -> Vec<u8> { let mut v = x.to_vec();
    for s in s0..s0 + steps { v = hs(&su.f, &[seed, &adrs(layer, tree, WOTS_HASH, kp, c, s), &v], N); } v }
fn sw_wsk(su: &Suite, seed: &[u8], sks: &[u8], layer: u32, tree: u64, kp: u32, c: u32) -> Vec<u8> { hs(&su.f, &[seed, &adrs(layer, tree, WOTS_PRF, kp, c, 0), sks], N) }
fn sw_xnode(su: &Suite, seed: &[u8], sks: &[u8], layer: u32, tree: u64, i: u32, z: u32) -> Vec<u8> {
    if z == 0 { let mut tmp = Vec::with_capacity(WL * N);
        for c in 0..WL as u32 { let sk = sw_wsk(su, seed, sks, layer, tree, i, c); tmp.extend(sw_chain(su, seed, &sk, layer, tree, i, c, 0, 15)); }
        hs(&su.h, &[seed, &adrs(layer, tree, WOTS_PK, i, 0, 0), &tmp], N) }
    else { let l = sw_xnode(su, seed, sks, layer, tree, 2 * i, z - 1); let r = sw_xnode(su, seed, sks, layer, tree, 2 * i + 1, z - 1);
        hs(&su.h, &[seed, &adrs(layer, tree, TREE, 0, z, i), &l, &r], N) } }
fn sw_fnode(su: &Suite, seed: &[u8], sks: &[u8], tree: u64, kp: u32, i: u32, z: u32) -> Vec<u8> {
    if z == 0 { let sk = hs(&su.f, &[seed, &adrs(0, tree, FORS_PRF, kp, 0, i), sks], N); hs(&su.f, &[seed, &adrs(0, tree, FORS_TREE, kp, 0, i), &sk], N) }
    else { let l = sw_fnode(su, seed, sks, tree, kp, 2 * i, z - 1); let r = sw_fnode(su, seed, sks, tree, kp, 2 * i + 1, z - 1);
        hs(&su.h, &[seed, &adrs(0, tree, FORS_TREE, kp, z, i), &l, &r], N) } }
fn sw_keygen(su: &Suite, sks: &[u8], seed: &[u8]) -> Vec<u8> { sw_xnode(su, seed, sks, (D - 1) as u32, 0, 0, HP as u32) }
fn sw_sign(su: &Suite, sks: &[u8], skp: &[u8], seed: &[u8], root: &[u8], m: &[u8]) -> Vec<u8> {
    let r = hs(&su.h, &[skp, seed, m], N); let dg = hs(&su.h, &[&r, seed, root, m], 34); let ix = split_digest(&dg);
    let mut sig = r.clone(); let mut roots = Vec::new();
    for i in 0..FK as u32 { let gi = i * FT as u32 + ix.fidx[i as usize];
        sig.extend(hs(&su.f, &[seed, &adrs(0, ix.itree, FORS_PRF, ix.ileaf, 0, gi), sks], N));
        for j in 0..FA as u32 { sig.extend(sw_fnode(su, seed, sks, ix.itree, ix.ileaf, (gi >> j) ^ 1, j)); }
        roots.extend(sw_fnode(su, seed, sks, ix.itree, ix.ileaf, i, FA as u32)); }
    let mut msg = hs(&su.h, &[seed, &adrs(0, ix.itree, FORS_ROOTS, ix.ileaf, 0, 0), &roots], N);
    for j in 0..D { let (l, t, kp) = (j as u32, ix.ltree[j], ix.lleaf[j]); let dd = base16(&msg);
        for c in 0..WL { let sk = sw_wsk(su, seed, sks, l, t, kp, c as u32); sig.extend(sw_chain(su, seed, &sk, l, t, kp, c as u32, 0, dd[c] as u32)); }
        for z in 0..HP as u32 { sig.extend(sw_xnode(su, seed, sks, l, t, (kp >> z) ^ 1, z)); }
        msg = sw_xnode(su, seed, sks, l, t, 0, HP as u32); }
    sig }
fn sw_verify(su: &Suite, seed: &[u8], root: &[u8], m: &[u8], sig: &[u8]) -> bool { if sig.len() != SIGLEN { return false; }
    let dg = hs(&su.h, &[&sig[0..N], seed, root, m], 34); let ix = split_digest(&dg); let mut roots = Vec::new();
    for i in 0..FK { let o = N + i * (FA + 1) * N; let gi = (i * FT) as u32 + ix.fidx[i];
        let mut nd = hs(&su.f, &[seed, &adrs(0, ix.itree, FORS_TREE, ix.ileaf, 0, gi), &sig[o..o + N]], N);
        for j in 0..FA { let au = &sig[o + (1 + j) * N..o + (2 + j) * N]; let a = adrs(0, ix.itree, FORS_TREE, ix.ileaf, (j + 1) as u32, gi >> (j + 1));
            nd = if (ix.fidx[i] >> j) & 1 == 0 { hs(&su.h, &[seed, &a, &nd, au], N) } else { hs(&su.h, &[seed, &a, au, &nd], N) }; }
        roots.extend(nd); }
    let mut msg = hs(&su.h, &[seed, &adrs(0, ix.itree, FORS_ROOTS, ix.ileaf, 0, 0), &roots], N);
    for j in 0..D { let (l, t, kp) = (j as u32, ix.ltree[j], ix.lleaf[j]); let o = HTOFF + j * (WL + HP) * N; let dd = base16(&msg); let mut tmp = Vec::new();
        for c in 0..WL { tmp.extend(sw_chain(su, seed, &sig[o + c * N..o + c * N + N], l, t, kp, c as u32, dd[c] as u32, 15 - dd[c] as u32)); }
        let mut nd = hs(&su.h, &[seed, &adrs(l, t, WOTS_PK, kp, 0, 0), &tmp], N);
        for z in 0..HP { let au = &sig[o + (WL + z) * N..o + (WL + z + 1) * N]; let a = adrs(l, t, TREE, 0, (z + 1) as u32, kp >> (z + 1));
            nd = if (kp >> z) & 1 == 0 { hs(&su.h, &[seed, &a, &nd, au], N) } else { hs(&su.h, &[seed, &a, au, &nd], N) }; }
        msg = nd; }
    &msg[..] == root }

// ================= the job graph on the atoms =================
// A node is one batch of equal-shaped hash calls run by one atom; an edge says a node needs digests of another node.
// Whenever an atom is free the scheduler gives it the ready node with the highest priority and sets the atom, by register writes
// only, to the width (one 1600-bit core or two 800-bit cores), round count, lengths and operation (plain hash or chain) the node needs.
#[derive(Clone, Copy, Debug)]
enum Nk { PrfMsg, HMsg, FSk(usize), FLeaf(usize), FLvl(usize, usize), FRoot, TSt(usize, usize, usize), WSig(usize),
          VFLeaf(usize), VFLvl(usize, usize), VFRoot, VCh(usize, usize), VTl(usize), VLv(usize, usize), FwH }
const C_MSG: usize = 0; const C_FORS: usize = 1; const C_TREE: usize = 2; const C_WOTS: usize = 3; const C_VFORS: usize = 4; const C_VHT: usize = 5; const C_FW: usize = 6; const NCAT: usize = 7;
const CNAME: [&str; NCAT] = ["msg_hashes", "fors", "ht_trees", "wots_sign", "vfy_fors", "vfy_ht", "fw_hash"];
// processor routines, for the layout profile
const NR: usize = 7; const RNAME: [&str; NR] = ["prf_fill(64B)", "chain_template_fill(64B)", "F_fill(64B)", "H_fill(80B)", "T_len_fill(608B)", "T_k_fill(576B)", "msg_hash_fill"];
fn rclass(k: Nk) -> usize { match k { Nk::FSk(_) => 0, Nk::TSt(_, 0, _) => 0, Nk::TSt(_, 1, _) | Nk::WSig(_) | Nk::VCh(_, _) => 1, Nk::FLeaf(_) | Nk::VFLeaf(_) => 2,
    Nk::FLvl(_, _) | Nk::VFLvl(_, _) | Nk::VLv(_, _) | Nk::TSt(_, 3, _) | Nk::TSt(_, 4, _) | Nk::TSt(_, 5, _) => 3, Nk::TSt(_, 2, _) | Nk::VTl(_) => 4,
    Nk::FRoot | Nk::VFRoot => 5, _ => 6 } }
static mut FWTURBO: bool = false;       // firmware pre-hash: false SHAKE256 (FIPS 205), true TurboSHAKE256 (NON-STANDARD modes)
static mut OLD: bool = false;            // true: the split-atom layout of ruty_sign.rs (byte chunks, one tree per node); false: word layout, trees paired
struct Node { k: Nk, cat: usize, prio: i32, npred: usize, succ: Vec<usize> }
struct G { v: Vec<Node> }
impl G { fn add(&mut self, k: Nk, cat: usize, prio: i32) -> usize { self.v.push(Node { k: k, cat: cat, prio: prio, npred: 0, succ: Vec::new() }); self.v.len() - 1 }
         fn edge(&mut self, a: usize, b: usize) { self.v[a].succ.push(b); self.v[b].npred += 1; } }
struct Tr { layer: u32, tree: u64, sk: Vec<u8>, ends: Vec<u8>, lv: Vec<Vec<u8>> }
fn new_tr(layer: u32, tree: u64) -> Tr { Tr { layer: layer, tree: tree, sk: vec![0; NCT * N], ends: vec![0; NCT * N], lv: vec![vec![0; 8 * N], vec![0; 4 * N], vec![0; 2 * N], vec![0; N]] } }
struct St { su: Suite, np: usize, tparts: usize, gs: usize,
    seed: Vec<u8>, sks: Vec<u8>, skp: Vec<u8>, root: Vec<u8>, m: Vec<u8>,
    r: Vec<u8>, ix: Idx, fsk: Vec<u8>, fl: Vec<Vec<u8>>, fpk: Vec<u8>, tr: Vec<Tr>,
    wready: bool, wgrp: Vec<Vec<(usize, usize)>>, wsig: Vec<u8>,
    sig: Vec<u8>, vf: Vec<u8>, vd: Vec<[u8; WL]>, vpk: Vec<u8>, vcur: Vec<u8>, vok: bool,
    fwpa: u64, fwva: *mut u8, fwlen: usize, fwdig: Vec<u8>, err: usize }
fn new_st(su: Suite, n: usize) -> St { St { su: su, np: n, tparts: 1, gs: 1, seed: vec![], sks: vec![], skp: vec![], root: vec![], m: vec![], r: vec![0; N],
    ix: Idx { fidx: [0; FK], itree: 0, ileaf: 0, ltree: [0; D], lleaf: [0; D] }, fsk: vec![0; FK * FT * N],
    fl: (0..=FA).map(|z| vec![0u8; FK * (FT >> z) * N]).collect(), fpk: vec![0; N], tr: Vec::new(),
    wready: false, wgrp: vec![Vec::new(); 16], wsig: vec![0; D * WL * N], sig: vec![], vf: vec![0; FK * N], vd: vec![[0u8; WL]; D], vpk: vec![0; D * WL * N],
    vcur: vec![0; D * N], vok: false, fwpa: 0, fwva: std::ptr::null_mut(), fwlen: 0, fwdig: vec![0; 64], err: 0 } }
fn prange(total: usize, parts: usize, p: usize) -> (usize, usize) { (p * total / parts, (p + 1) * total / parts) }

#[derive(Clone, Copy)] struct Job { g: Alg, dom: u8, dual: bool, len: usize, olen: usize, cend: u32, cnt: usize, pc: usize, inp: u64 }
impl Job { fn nb(&self) -> usize { if self.dual { 4 } else { 8 } }
    fn wpm(&self) -> usize { (self.len + self.nb() - 1) / self.nb() } fn wpo(&self) -> usize { (self.olen + self.nb() - 1) / self.nb() }
    fn nj(&self) -> usize { if self.dual { self.pc / 2 } else { self.pc } }
    fn ibytes(&self) -> usize { self.nj() * self.wpm() * 8 } fn obytes(&self) -> usize { self.nj() * self.wpo() * 8 } }
// role h = false: a one-block call (PRF or F) hashed with su.f; true: H, T_l, H_msg, PRF_msg hashed with su.h
fn job(st: &St, h: bool, len: usize, olen: usize, cend: u32, cnt: usize) -> Job { let hf = if h { st.su.h } else { st.su.f }; let dual = hf.dual;
    Job { g: hf.g, dom: hf.dom, dual: dual, len: len, olen: olen, cend: cend, cnt: cnt, pc: if dual && cnt % 2 == 1 { cnt + 1 } else { cnt }, inp: 0 } }
// byte layout the atom reads and writes: one 64-bit word = one lane; split atom: a word carries 4 bytes of each of two jobs (core 0 = low half)
struct Wr { base: *mut u8, dual: bool, wpm: usize }
impl Wr { #[inline(always)] unsafe fn put(&self, k: usize, off: usize, src: &[u8]) { let n = src.len();
        if !self.dual { std::ptr::copy_nonoverlapping(src.as_ptr(), self.base.add(k * self.wpm * 8 + off), n); return; }
        let b = self.base.add((k / 2) * self.wpm * 8 + (k % 2) * 4); let mut c = 0;
        while c < n { let p = off + c; let q = p % 4; let ch = if 4 - q < n - c { 4 - q } else { n - c };
            std::ptr::copy_nonoverlapping(src.as_ptr().add(c), b.add((p / 4) * 8 + q), ch); c += ch; } } }
#[inline(always)] unsafe fn getd(base: *const u8, jb: &Job, k: usize, dst: &mut [u8]) { let n = dst.len(); let wpo = jb.wpo();
    if !jb.dual { std::ptr::copy_nonoverlapping(base.add(k * wpo * 8), dst.as_mut_ptr(), n); return; }
    if !OLD { let b = base.add((k / 2) * wpo * 8) as *const u64; let sh = 32 * (k % 2) as u32; let d = dst.as_mut_ptr();
        for i in 0..n / 4 { std::ptr::write_unaligned((d as *mut u32).add(i), (std::ptr::read_unaligned(b.add(i)) >> sh) as u32); }
        if n % 4 != 0 { let i = n / 4; let v = (std::ptr::read_unaligned(b.add(i)) >> sh) as u32; for j in 0..n % 4 { *d.add(4 * i + j) = (v >> (8 * j)) as u8; } } return; }
    let b = base.add((k / 2) * wpo * 8 + (k % 2) * 4); let mut c = 0; while c < n { let ch = if n - c < 4 { n - c } else { 4 }; std::ptr::copy_nonoverlapping(b.add((c / 4) * 8), dst.as_mut_ptr().add(c), ch); c += 4; } }
// ---- split-atom word layout: every job is built as little-endian 32-bit words, two jobs are merged into each 64-bit word
type Wv = [u32; 160];
#[inline(always)] fn aw(layer: u32, tree: u64, typ: u32, kp: u32, a: u32, b: u32) -> [u32; 8] {
    [layer.swap_bytes(), 0, ((tree >> 32) as u32).swap_bytes(), (tree as u32).swap_bytes(), typ.swap_bytes(), kp.swap_bytes(), a.swap_bytes(), b.swap_bytes()] }
#[inline(always)] unsafe fn hd(w: &mut Wv, sw: &[u32; 4], ad: [u32; 8]) { for i in 0..4 { *w.get_unchecked_mut(i) = *sw.get_unchecked(i); } for i in 0..8 { *w.get_unchecked_mut(4 + i) = *ad.get_unchecked(i); } }
#[inline(always)] unsafe fn ld(w: &mut Wv, at: usize, src: *const u8, nw: usize) { for i in 0..nw { *w.get_unchecked_mut(at + i) = std::ptr::read_unaligned((src as *const u32).add(i)); } }
unsafe fn fill_split<F: Fn(usize, &mut Wv)>(base: *mut u8, jb: &Job, gen: F) {
    let wpm = jb.wpm(); let mut a: Wv = [0u32; 160]; let mut b: Wv = [0u32; 160];
    for p in 0..jb.pc / 2 { let ca = 2 * p; let cb = if ca + 1 < jb.cnt { ca + 1 } else { jb.cnt - 1 };
        gen(ca, &mut a); gen(cb, &mut b); let o = base.add(p * wpm * 8) as *mut u64;
        for i in 0..wpm { std::ptr::write_unaligned(o.add(i), (*a.get_unchecked(i) as u64) | ((*b.get_unchecked(i) as u64) << 32)); } } }
fn wprep(st: &mut St) { if st.wready { return; } for m in 0..16 { st.wgrp[m].clear(); }
    for j in 0..D { let dd = if j == 0 { base16(&st.fpk) } else { base16(&st.tr[j - 1].lv[3]) }; for c in 0..WL { st.wgrp[dd[c] as usize].push((j, c)); } }
    st.wready = true; }
fn vlist(st: &St, j: usize, g: usize) -> Vec<usize> { let dd = &st.vd[j];
    if st.su.f.dual { let _ = g; let mut by: Vec<Vec<usize>> = vec![Vec::new(); 15]; for c in 0..WL { if dd[c] != 15 { by[dd[c] as usize].push(c); } }
        let mut o = Vec::new(); for v in by.iter() { for &c in v.iter() { o.push(c); } if v.len() % 2 == 1 { o.push(v[v.len() - 1]); } } o }
    else { let all: Vec<usize> = (0..WL).filter(|&c| dd[c] != 15).collect(); let (a, b) = prange(all.len(), st.np, g); all[a..b].to_vec() } }
fn vprep(st: &mut St, j: usize, msg: &[u8]) { st.vd[j] = base16(msg); let o = HTOFF + j * (WL + HP) * N;
    for c in 0..WL { if st.vd[j][c] == 15 { let s = st.sig[o + c * N..o + c * N + N].to_vec(); st.vpk[(j * WL + c) * N..(j * WL + c + 1) * N].copy_from_slice(&s); } } }
// trees of node group g: g*gs .. ; per-tree job count of stage s
fn tcount(st: &St, s: usize) -> usize { match s { 0 | 1 => NCT, 2 => 8, _ => 8 >> (s - 2) } }
fn tgroup(st: &St, g: usize) -> (usize, usize) { let a = g * st.gs; let b = if a + st.gs < st.tr.len() { a + st.gs } else { st.tr.len() }; (a, b) }
// stage-s job c of group g (part p) -> (tree index, job index inside that tree)
fn tjob(st: &St, g: usize, s: usize, p: usize, c: usize) -> (usize, usize) { let per = tcount(st, s); let (t0, _) = tgroup(st, g);
    if s < 2 { let (a, _) = prange(NCT, st.tparts, p); let x = if st.tparts > 1 { a + c } else { c }; (t0 + x / per, x % per) } else { (t0 + c / per, c % per) } }
fn tjobs(st: &St, g: usize, s: usize, p: usize) -> usize { let (t0, t1) = tgroup(st, g); if s < 2 && st.tparts > 1 { let (a, b) = prange(NCT, st.tparts, p); b - a } else { (t1 - t0) * tcount(st, s) } }

// write the messages of node k into the atom's input buffer; returns the batch shape (cnt = 0: nothing to hash)
unsafe fn fill(st: &mut St, k: Nk, base: *mut u8) -> Job {
    if let Nk::WSig(_) = k { wprep(st); }
    let st: &St = &*st; let seed = &st.seed[..]; let ix = &st.ix;
    if let Nk::FwH = k { let s = if FWTURBO { hf_of("turbo") } else { hf_of("std") }; return Job { g: s.g, dom: s.dom, dual: false, len: st.fwlen, olen: 64, cend: 0, cnt: 1, pc: 1, inp: st.fwpa }; }
    let sw: [u32; 4] = [std::ptr::read_unaligned(seed.as_ptr() as *const u32), std::ptr::read_unaligned((seed.as_ptr() as *const u32).add(1)),
                        std::ptr::read_unaligned((seed.as_ptr() as *const u32).add(2)), std::ptr::read_unaligned((seed.as_ptr() as *const u32).add(3))];
    // describe job c: (adrs words, first data pointer, its length in bytes, optional second data pointer of 16 bytes)
    macro_rules! go { ($jb:expr, $c:ident, $body:expr) => {{ let jb: Job = $jb;
        if jb.cnt == 0 { return jb; }
        if jb.dual && !OLD && jb.len % 4 == 0 {
            fill_split(base, &jb, |$c: usize, w: &mut Wv| { let (ad, p1, l1, p2): ([u32; 8], *const u8, usize, *const u8) = $body; hd(w, &sw, ad); ld(w, 12, p1, l1 / 4); if !p2.is_null() { ld(w, 12 + l1 / 4, p2, 4); } }); }
        else { let wr = Wr { base: base, dual: jb.dual, wpm: jb.wpm() };
            for kk in 0..jb.pc { let $c = if kk < jb.cnt { kk } else { jb.cnt - 1 }; let (ad, p1, l1, p2): ([u32; 8], *const u8, usize, *const u8) = $body;
                let mut ab = [0u8; 32]; for i in 0..8 { ab[4 * i..4 * i + 4].copy_from_slice(&ad[i].to_le_bytes()); }
                wr.put(kk, 0, seed); wr.put(kk, 16, &ab); wr.put(kk, 48, std::slice::from_raw_parts(p1, l1)); if !p2.is_null() { wr.put(kk, 48 + l1, std::slice::from_raw_parts(p2, 16)); } } }
        jb }} }
    let nul: *const u8 = std::ptr::null();
    match k {
        Nk::PrfMsg => { let jb = job(st, true, 32 + st.m.len(), N, 0, 1); let w = Wr { base: base, dual: jb.dual, wpm: jb.wpm() };
            for s in 0..jb.pc { w.put(s, 0, &st.skp); w.put(s, 16, &st.seed); w.put(s, 32, &st.m); } jb }
        Nk::HMsg => { let jb = job(st, true, 48 + st.m.len(), 34, 0, 1); let w = Wr { base: base, dual: jb.dual, wpm: jb.wpm() };
            for s in 0..jb.pc { w.put(s, 0, &st.r); w.put(s, 16, &st.seed); w.put(s, 32, &st.root); w.put(s, 48, &st.m); } jb }
        Nk::FSk(p) => { let (a, b) = prange(FK, st.np, p);
            go!(job(st, false, 64, N, 0, (b - a) * FT), c, { let gi = a * FT + c; (aw(0, ix.itree, FORS_PRF, ix.ileaf, 0, gi as u32), st.sks.as_ptr(), 16, nul) }) }
        Nk::FLeaf(p) => { let (a, b) = prange(FK, st.np, p);
            go!(job(st, false, 64, N, 0, (b - a) * FT), c, { let gi = a * FT + c; (aw(0, ix.itree, FORS_TREE, ix.ileaf, 0, gi as u32), st.fsk.as_ptr().add(gi * N), 16, nul) }) }
        Nk::FLvl(z, p) => { let (a, b) = prange(FK, st.np, p); let per = FT >> z;
            go!(job(st, true, 80, N, 0, (b - a) * per), c, { let gi = a * per + c; (aw(0, ix.itree, FORS_TREE, ix.ileaf, z as u32, gi as u32), st.fl[z - 1].as_ptr().add(2 * gi * N), 32, nul) }) }
        Nk::FRoot => { go!(job(st, true, 48 + FK * N, N, 0, 1), c, { (aw(0, ix.itree, FORS_ROOTS, ix.ileaf, 0, 0), st.fl[FA].as_ptr(), FK * N, nul) }) }
        Nk::TSt(g, s, p) => { let cnt = tjobs(st, g, s, p);
            match s {
                0 => go!(job(st, false, 64, N, 0, cnt), c, { let (t, ch) = tjob(st, g, 0, p, c); let tr = &st.tr[t];
                         (aw(tr.layer, tr.tree, WOTS_PRF, (ch / WL) as u32, (ch % WL) as u32, 0), st.sks.as_ptr(), 16, nul) }),
                1 => go!(job(st, false, 64, N, 15, cnt), c, { let (t, ch) = tjob(st, g, 1, p, c); let tr = &st.tr[t];
                         (aw(tr.layer, tr.tree, WOTS_HASH, (ch / WL) as u32, (ch % WL) as u32, 0), tr.sk.as_ptr().add(ch * N), 16, nul) }),
                2 => go!(job(st, true, 48 + WL * N, N, 0, cnt), c, { let (t, i) = tjob(st, g, 2, p, c); let tr = &st.tr[t];
                         (aw(tr.layer, tr.tree, WOTS_PK, i as u32, 0, 0), tr.ends.as_ptr().add(i * WL * N), WL * N, nul) }),
                _ => { let z = s - 2; go!(job(st, true, 80, N, 0, cnt), c, { let (t, i) = tjob(st, g, s, p, c); let tr = &st.tr[t];
                         (aw(tr.layer, tr.tree, TREE, 0, z as u32, i as u32), tr.lv[z - 1].as_ptr().add(2 * i * N), 32, nul) }) } } }
        Nk::WSig(m) => { let grp = &st.wgrp[m];
            go!(job(st, false, 64, N, m as u32, grp.len()), c, { let (j, ch) = grp[c]; let kp = ix.lleaf[j];
                (aw(j as u32, ix.ltree[j], WOTS_HASH, kp, ch as u32, 0), st.tr[j].sk.as_ptr().add((kp as usize * WL + ch) * N), 16, nul) }) }
        Nk::VFLeaf(p) => { let (a, b) = prange(FK, st.np, p);
            go!(job(st, false, 64, N, 0, b - a), c, { let i = a + c; (aw(0, ix.itree, FORS_TREE, ix.ileaf, 0, (i * FT) as u32 + ix.fidx[i]), st.sig.as_ptr().add(N + i * (FA + 1) * N), 16, nul) }) }
        Nk::VFLvl(z, p) => { let (a, b) = prange(FK, st.np, p);
            go!(job(st, true, 80, N, 0, b - a), c, { let i = a + c; let o = N + i * (FA + 1) * N + z * N; let gi = (i * FT) as u32 + ix.fidx[i];
                let (x, y) = if (ix.fidx[i] >> (z - 1)) & 1 == 0 { (st.vf.as_ptr().add(i * N), st.sig.as_ptr().add(o)) } else { (st.sig.as_ptr().add(o), st.vf.as_ptr().add(i * N)) };
                (aw(0, ix.itree, FORS_TREE, ix.ileaf, z as u32, gi >> z), x, 16, y) }) }
        Nk::VFRoot => { go!(job(st, true, 48 + FK * N, N, 0, 1), c, { (aw(0, ix.itree, FORS_ROOTS, ix.ileaf, 0, 0), st.vf.as_ptr(), FK * N, nul) }) }
        Nk::VCh(j, g) => { let l = vlist(st, j, g); let o = HTOFF + j * (WL + HP) * N;
            go!(job(st, false, 64, N, 15, l.len()), c, { let ch = l[c];
                (aw(j as u32, ix.ltree[j], WOTS_HASH, ix.lleaf[j], ch as u32, st.vd[j][ch] as u32), st.sig.as_ptr().add(o + ch * N), 16, nul) }) }
        Nk::VTl(j) => { go!(job(st, true, 48 + WL * N, N, 0, 1), c, { (aw(j as u32, ix.ltree[j], WOTS_PK, ix.lleaf[j], 0, 0), st.vpk.as_ptr().add(j * WL * N), WL * N, nul) }) }
        Nk::VLv(j, z) => { let kp = ix.lleaf[j]; let o = HTOFF + j * (WL + HP) * N + (WL + z - 1) * N;
            go!(job(st, true, 80, N, 0, 1), c, { let (x, y) = if (kp >> (z - 1)) & 1 == 0 { (st.vcur.as_ptr().add(j * N), st.sig.as_ptr().add(o)) } else { (st.sig.as_ptr().add(o), st.vcur.as_ptr().add(j * N)) };
                (aw(j as u32, ix.ltree[j], TREE, 0, z as u32, kp >> z), x, 16, y) }) }
        Nk::FwH => { let s = if FWTURBO { hf_of("turbo") } else { hf_of("std") }; Job { g: s.g, dom: s.dom, dual: false, len: st.fwlen, olen: 64, cend: 0, cnt: 1, pc: 1, inp: st.fwpa } } } }

// read the digests of node k out of the atom's output buffer
unsafe fn store(st: &mut St, k: Nk, jb: &Job, ob: *const u8) {
    let mut d = [0u8; 64];
    match k {
        Nk::PrfMsg => { getd(ob, jb, 0, &mut d[..N]); st.r = d[..N].to_vec(); }
        Nk::HMsg => { getd(ob, jb, 0, &mut d[..34]); st.ix = split_digest(&d[..34]);
            if st.tr.len() == D { for t in 0..D { st.tr[t].tree = st.ix.ltree[t]; } } }
        Nk::FSk(p) | Nk::FLeaf(p) => { let (a, _) = prange(FK, st.np, p); let dst = match k { Nk::FSk(_) => &mut st.fsk, _ => &mut st.fl[0] };
            for c in 0..jb.cnt { getd(ob, jb, c, &mut dst[(a * FT + c) * N..(a * FT + c + 1) * N]); } }
        Nk::FLvl(z, p) => { let (a, _) = prange(FK, st.np, p); let per = FT >> z; for c in 0..jb.cnt { getd(ob, jb, c, &mut st.fl[z][(a * per + c) * N..(a * per + c + 1) * N]); } }
        Nk::FRoot => { getd(ob, jb, 0, &mut d[..N]); st.fpk = d[..N].to_vec(); }
        Nk::TSt(g, s, p) => { for c in 0..jb.cnt { let (t, i) = tjob(st, g, s, p, c); let tr = &mut st.tr[t];
                let dst = match s { 0 => &mut tr.sk[i * N..(i + 1) * N], 1 => &mut tr.ends[i * N..(i + 1) * N], _ => &mut tr.lv[s - 2][i * N..(i + 1) * N] }; getd(ob, jb, c, dst); } }
        Nk::WSig(m) => { for c in 0..jb.cnt { let (j, ch) = st.wgrp[m][c]; getd(ob, jb, c, &mut st.wsig[(j * WL + ch) * N..(j * WL + ch + 1) * N]); } }
        Nk::VFLeaf(p) | Nk::VFLvl(_, p) => { let (a, _) = prange(FK, st.np, p); for c in 0..jb.cnt { getd(ob, jb, c, &mut st.vf[(a + c) * N..(a + c + 1) * N]); } }
        Nk::VFRoot => { getd(ob, jb, 0, &mut d[..N]); st.fpk = d[..N].to_vec(); let f = st.fpk.clone(); vprep(st, 0, &f); }
        Nk::VCh(j, g) => { let l = vlist(st, j, g); for c in 0..jb.cnt { let ch = l[c]; getd(ob, jb, c, &mut st.vpk[(j * WL + ch) * N..(j * WL + ch + 1) * N]); } }
        Nk::VTl(j) | Nk::VLv(j, _) => { getd(ob, jb, 0, &mut d[..N]); st.vcur[j * N..j * N + N].copy_from_slice(&d[..N]);
            if let Nk::VLv(_, 3) = k { if j + 1 < D { vprep(st, j + 1, &d[..N]); } else { st.vok = &d[..N] == &st.root[..]; } } }
        Nk::FwH => { getd(ob, jb, 0, &mut d[..64]); st.fwdig = d.to_vec(); } } }

struct Stats { ns: f64, proc_ns: [f64; NCAT], atom_ns: [f64; NCAT], end_ns: [f64; NCAT], batches: usize, asm_ns: f64,
               fill_ns: [f64; NR], coll_ns: [f64; NR], calls: [f64; NR], pads: [f64; NR], flush_ns: f64, regs_ns: f64 }
fn new_stats() -> Stats { Stats { ns: 0.0, proc_ns: [0.0; NCAT], atom_ns: [0.0; NCAT], end_ns: [0.0; NCAT], batches: 0, asm_ns: 0.0,
    fill_ns: [0.0; NR], coll_ns: [0.0; NR], calls: [0.0; NR], pads: [0.0; NR], flush_ns: 0.0, regs_ns: 0.0 } }
unsafe fn run_graph(engs: &[Eng], st: &mut St, g: &mut G) -> Stats {
    let n = engs.len(); let mut s = new_stats();
    let mut ready: Vec<usize> = (0..g.v.len()).filter(|&i| g.v[i].npred == 0).collect();
    let mut at: Vec<Option<(usize, Job)>> = vec![None; n]; let mut since = vec![0.0f64; n]; let mut left = g.v.len(); let mut spins = 0u64;
    let t0 = now_ns();
    while left > 0 { spins += 1; if spins > 400_000_000 { st.err += 1000; println!("RESULT error=graph_timeout left={}", left); break; }
        let mut busy = 0;
        for ai in 0..n { let e = &engs[ai];
            if let Some((ni, jb)) = at[ai] { if let Some(stt) = e.finished() { let td = now_ns(); let cat = g.v[ni].cat; let r = rclass(g.v[ni].k); s.atom_ns[cat] += td - since[ai];
                    if stt & 0x72 != 2 { st.err += 1; println!("RESULT error=atom_status atom={} status={:#x} node={:?}", ai, stt, g.v[ni].k); }
                    let ob = e.buf(1).0; from_device(ob, jb.obytes()); let tq = now_ns(); s.flush_ns += tq - td; store(st, g.v[ni].k, &jb, ob);
                    for q in 0..g.v[ni].succ.len() { let sx = g.v[ni].succ[q]; g.v[sx].npred -= 1; if g.v[sx].npred == 0 { ready.push(sx); } }
                    left -= 1; at[ai] = None; let te = now_ns(); s.coll_ns[r] += te - tq; s.proc_ns[cat] += te - td; s.end_ns[cat] = te - t0; } else { busy += 1; } }
            while at[ai].is_none() && !ready.is_empty() {
                let mut bi = 0; for i in 1..ready.len() { if g.v[ready[i]].prio > g.v[ready[bi]].prio { bi = i; } } let ni = ready.remove(bi);
                let tf = now_ns(); let cat = g.v[ni].cat; let r = rclass(g.v[ni].k); let (ib, ipa) = e.buf(0); let jb = fill(st, g.v[ni].k, ib); let tg = now_ns();
                s.fill_ns[r] += tg - tf; s.calls[r] += jb.cnt as f64; s.pads[r] += (jb.pc - jb.cnt) as f64;
                if jb.cnt == 0 { for q in 0..g.v[ni].succ.len() { let sx = g.v[ni].succ[q]; g.v[sx].npred -= 1; if g.v[sx].npred == 0 { ready.push(sx); } }
                    left -= 1; let te = now_ns(); s.proc_ns[cat] += te - tf; s.end_ns[cat] = te - t0; continue; }
                let inp = if jb.inp != 0 { to_device(st.fwva, st.fwlen); win(e, jb.inp, st.fwlen); jb.inp } else { to_device(ib, jb.ibytes()); ipa };
                let th = now_ns(); s.flush_ns += th - tg;
                let nb = jb.nb() as u32; e.wr(LEN, jb.len as u32 | ((jb.dom as u32) << 24)); e.wr(OLEN, jb.olen as u32);
                e.wr(ALG, jb.g.first | (jb.g.last << 8) | ((jb.dual as u32) << 16) | ((jb.g.rate as u32 / nb) << 24));
                if jb.cend != 0 { e.wr(0x44, 1 | ((47 / nb) << 8) | ((47 % nb) << 16) | ((48 / nb) << 20)); e.wr(0x48, jb.cend); } else { e.wr(0x44, 0); }
                e.start(inp, e.buf(1).1, jb.nj()); since[ai] = now_ns(); s.regs_ns += since[ai] - th; s.proc_ns[cat] += since[ai] - tf; at[ai] = Some((ni, jb)); s.batches += 1; busy += 1; } }
        if busy == 0 && ready.is_empty() && left > 0 { st.err += 1000; println!("RESULT error=graph_stuck left={}", left); break; } }
    for e in engs.iter() { e.wr(0x44, 0); }
    s.ns = now_ns() - t0; s }

// ---------------- the three operations ----------------
fn keygen_graph(st: &mut St) -> G { let mut g = G { v: Vec::new() }; st.tr = vec![new_tr((D - 1) as u32, 0)]; st.tparts = st.np; st.gs = 1;
    let mut prev: Vec<usize> = Vec::new();
    for s in 0..6 { let parts = if s < 2 { st.tparts } else { 1 }; let cur: Vec<usize> = (0..parts).map(|p| g.add(Nk::TSt(0, s, p), C_TREE, s as i32)).collect();
        for &a in &prev { for &b in &cur { g.edge(a, b); } } prev = cur; }
    g }
fn sign_graph(st: &mut St) -> G { let mut g = G { v: Vec::new() }; st.tparts = 1; st.wready = false;
    st.gs = if st.su.h.dual && !unsafe { OLD } { 2 } else { 1 };        // pure split: two trees per node, so no call is paired with a copy of itself
    st.tr = (0..D).map(|j| new_tr(j as u32, 0)).collect();
    let p = g.add(Nk::PrfMsg, C_MSG, 100); let h = g.add(Nk::HMsg, C_MSG, 100); g.edge(p, h);
    let froot = g.add(Nk::FRoot, C_FORS, 9);
    for q in 0..st.np { let a = g.add(Nk::FSk(q), C_FORS, 0); let b = g.add(Nk::FLeaf(q), C_FORS, 1); g.edge(h, a); g.edge(a, b); let mut pv = b;
        for z in 1..=FA { let x = g.add(Nk::FLvl(z, q), C_FORS, 1 + z as i32); g.edge(pv, x); pv = x; } g.edge(pv, froot); }
    let ws: Vec<usize> = (1..16).map(|m| g.add(Nk::WSig(m), C_WOTS, 20)).collect(); for &w in &ws { g.edge(froot, w); }
    let ng = (D + st.gs - 1) / st.gs;
    for t in 0..ng { let mut pv = h; for s in 0..6 { let x = g.add(Nk::TSt(t, s, 0), C_TREE, s as i32); g.edge(pv, x); pv = x; } for &w in &ws { g.edge(pv, w); } }
    g }
fn verify_graph(st: &mut St) -> G { let mut g = G { v: Vec::new() }; st.vok = false;
    let h = g.add(Nk::HMsg, C_MSG, 100); let fr = g.add(Nk::VFRoot, C_VFORS, 10);
    for q in 0..st.np { let a = g.add(Nk::VFLeaf(q), C_VFORS, 1); g.edge(h, a); let mut pv = a;
        for z in 1..=FA { let x = g.add(Nk::VFLvl(z, q), C_VFORS, 1 + z as i32); g.edge(pv, x); pv = x; } g.edge(pv, fr); }
    let mut pv = fr; let ng = if st.su.f.dual { 1 } else { st.np };
    for j in 0..D { let chs: Vec<usize> = (0..ng).map(|q| g.add(Nk::VCh(j, q), C_VHT, 20 + j as i32)).collect(); let tl = g.add(Nk::VTl(j), C_VHT, 20 + j as i32);
        for &c in &chs { g.edge(pv, c); g.edge(c, tl); } let mut x = tl; for z in 1..=HP { let y = g.add(Nk::VLv(j, z), C_VHT, 20 + j as i32); g.edge(x, y); x = y; } pv = x; }
    g }
unsafe fn atom_keygen(engs: &[Eng], su: Suite, sks: &[u8], seed: &[u8]) -> (Vec<u8>, Stats, usize) {
    let mut st = new_st(su, engs.len()); st.sks = sks.to_vec(); st.seed = seed.to_vec(); let mut g = keygen_graph(&mut st); let s = run_graph(engs, &mut st, &mut g);
    (st.tr[0].lv[3].clone(), s, st.err) }
unsafe fn atom_sign(engs: &[Eng], su: Suite, sks: &[u8], skp: &[u8], seed: &[u8], root: &[u8], m: &[u8]) -> (Vec<u8>, Stats, usize, bool) {
    let mut st = new_st(su, engs.len()); st.sks = sks.to_vec(); st.skp = skp.to_vec(); st.seed = seed.to_vec(); st.root = root.to_vec(); st.m = m.to_vec();
    let t0 = now_ns(); let mut g = sign_graph(&mut st); let mut s = run_graph(engs, &mut st, &mut g); let ta = now_ns();
    // assembly on the A53: R, SIG_FORS (secret value + 6 authentication nodes per tree), SIG_HT (22 x (35 chain values + 3 authentication nodes))
    let mut sig = Vec::with_capacity(SIGLEN); sig.extend_from_slice(&st.r); let ix = &st.ix;
    for i in 0..FK { let gi = i * FT + ix.fidx[i] as usize; sig.extend_from_slice(&st.fsk[gi * N..gi * N + N]);
        for j in 0..FA { let x = (gi >> j) ^ 1; sig.extend_from_slice(&st.fl[j][x * N..x * N + N]); } }
    wprep(&mut st); let ix = &st.ix;
    for j in 0..D { let kp = ix.lleaf[j] as usize; let dd = if j == 0 { base16(&st.fpk) } else { base16(&st.tr[j - 1].lv[3]) };
        for c in 0..WL { if dd[c] == 0 { sig.extend_from_slice(&st.tr[j].sk[(kp * WL + c) * N..(kp * WL + c + 1) * N]); } else { sig.extend_from_slice(&st.wsig[(j * WL + c) * N..(j * WL + c + 1) * N]); } }
        for z in 0..HP { let x = (kp >> z) ^ 1; sig.extend_from_slice(&st.tr[j].lv[z][x * N..x * N + N]); } }
    let te = now_ns(); s.asm_ns = te - ta; s.ns = te - t0;
    let top_ok = &st.tr[D - 1].lv[3][..] == root;              // the top tree's root must be PK.root
    (sig, s, st.err, top_ok) }
// parts: how many atoms the verifier spreads each layer's chains (and the FORS trees) over (0 = all atoms)
unsafe fn atom_verify(engs: &[Eng], su: Suite, seed: &[u8], root: &[u8], m: &[u8], sig: &[u8], parts: usize) -> (bool, Stats, usize) {
    if sig.len() != SIGLEN { return (false, new_stats(), 0); }
    let mut st = new_st(su, if parts == 0 { engs.len() } else { parts }); st.seed = seed.to_vec(); st.root = root.to_vec(); st.m = m.to_vec(); st.sig = sig.to_vec(); st.r = sig[0..N].to_vec();
    let t0 = now_ns(); let mut g = verify_graph(&mut st); let mut s = run_graph(engs, &mut st, &mut g); s.ns = now_ns() - t0; (st.vok, s, st.err) }
unsafe fn atom_fw_hash(engs: &[Eng], va: *mut u8, pa: u64, len: usize) -> (Vec<u8>, Stats, usize) {
    let mut st = new_st(suite_of("std"), engs.len()); st.fwva = va; st.fwpa = pa; st.fwlen = len; let mut g = G { v: Vec::new() }; g.add(Nk::FwH, C_FW, 0);
    let s = run_graph(&engs[0..1], &mut st, &mut g); for e in engs.iter() { e.wr(WBASE, (e.pa >> 12) as u32); e.wr(WLIMIT, ((e.pa + PAGE as u64) >> 12) as u32); }
    (st.fwdig.clone(), s, st.err) }

// ================= set-up of the atoms and buffers =================
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
unsafe fn setup(n: usize, fwlen: usize) -> Option<(Vec<Eng>, *mut u8, u64)> {
    let fd = open(b"/dev/mem\0".as_ptr() as *const c_char, 2 | 0x101000); if fd < 0 { println!("RESULT error=open_dev_mem"); return None; }
    let mut pm = File::open("/proc/self/pagemap").unwrap();
    let hf = open(b"/dev/dma_heap/reserved\0".as_ptr() as *const c_char, 2); if hf < 0 { println!("RESULT error=open_dma_heap"); return None; }
    let mut engs = Vec::new();
    for i in 0..n { let reg = mmap(std::ptr::null_mut(), 4096, 3, 1, fd, 0xA000_0000i64 + 0x1000 * i as i64) as *mut u32;
        let (va, pa) = match heap_buf(hf, &mut pm, PAGE) { Some(x) => x, None => { println!("RESULT error=atom_buffer"); return None; } };
        let g = Eng { reg: reg, va: va, pa: pa }; g.wr(WBASE, (pa >> 12) as u32); g.wr(WLIMIT, ((pa + PAGE as u64) >> 12) as u32); g.wr(0x44, 0); engs.push(g); }
    let (fva, fpa) = match heap_buf(hf, &mut pm, fwlen) { Some(x) => x, None => { println!("RESULT error=fw_buffer"); return None; } };
    Some((engs, fva, fpa)) }
#[cfg(not(target_arch = "aarch64"))]
unsafe fn setup(n: usize, fwlen: usize) -> Option<(Vec<Eng>, *mut u8, u64)> {
    let mut engs = Vec::new();
    for _ in 0..n { let reg = Box::leak(vec![0u32; 64].into_boxed_slice()).as_mut_ptr(); let va = Box::leak(vec![0u8; PAGE + 64].into_boxed_slice()).as_mut_ptr();
        let va = ((va as usize + 63) & !63) as *mut u8; engs.push(Eng { reg: reg, va: va, pa: va as u64 }); }
    let fva = Box::leak(vec![0u8; fwlen].into_boxed_slice()).as_mut_ptr(); Some((engs, fva, fva as u64)) }

fn stats(v: &[f64]) -> String { let mut s = v.to_vec(); s.sort_by(|a, b| a.partial_cmp(b).unwrap()); let n = s.len();
    // ruty_sign5: the only change from ruty_sign4 is the field samples_ms, the n individual times in the order they were taken
    let all: Vec<String> = v.iter().map(|x| format!("{:.4}", x)).collect();
    format!("n={} min_ms={:.4} q1_ms={:.4} median_ms={:.4} q3_ms={:.4} max_ms={:.4} spread_max_over_min={:.4} samples_ms={}", n, s[0], s[n / 4], s[n / 2], s[(3 * n) / 4], s[n - 1], s[n - 1] / s[0], all.join(",")) }
fn bd(name: &str, v: &str, ss: &[Stats], cats: &[usize]) { let k = ss.len() as f64;
    let mut line = format!("RESULT breakdown v={} op={} mean_total_ms={:.4} mean_batches={:.1} assembly_ms={:.4}", v, name,
        ss.iter().map(|s| s.ns).sum::<f64>() / k / 1e6, ss.iter().map(|s| s.batches as f64).sum::<f64>() / k, ss.iter().map(|s| s.asm_ns).sum::<f64>() / k / 1e6);
    let mut ptot = 0.0;
    for &c in cats { let e = ss.iter().map(|s| s.end_ns[c]).sum::<f64>() / k / 1e6; let a = ss.iter().map(|s| s.atom_ns[c]).sum::<f64>() / k / 1e6; let p = ss.iter().map(|s| s.proc_ns[c]).sum::<f64>() / k / 1e6; ptot += p;
        line.push_str(&format!(" | {}: done_at_ms={:.4} atom_busy_ms={:.4} proc_ms={:.4}", CNAME[c], e, a, p)); }
    line.push_str(&format!(" | processor_layout_total_ms={:.4}", ptot + ss.iter().map(|s| s.asm_ns).sum::<f64>() / k / 1e6)); println!("{}", line); }
fn prof(name: &str, v: &str, ss: &[Stats]) { let k = ss.len() as f64; let mut tot = 0.0;
    for r in 0..NR { let f = ss.iter().map(|s| s.fill_ns[r]).sum::<f64>() / k; let c = ss.iter().map(|s| s.coll_ns[r]).sum::<f64>() / k; let n = ss.iter().map(|s| s.calls[r]).sum::<f64>() / k;
        let p = ss.iter().map(|s| s.pads[r]).sum::<f64>() / k; tot += f + c; if n == 0.0 { continue; }
        println!("RESULT profile v={} op={} routine={} calls={:.0} pad_copies={:.0} fill_ms={:.4} collect_ms={:.4} fill_ns_per_call={:.1} collect_ns_per_call={:.1}", v, name, RNAME[r], n, p, f / 1e6, c / 1e6, f / n, c / n); }
    let fl = ss.iter().map(|s| s.flush_ns).sum::<f64>() / k; let rg = ss.iter().map(|s| s.regs_ns).sum::<f64>() / k; let asm = ss.iter().map(|s| s.asm_ns).sum::<f64>() / k;
    let calls = (0..NR).map(|r| ss.iter().map(|s| s.calls[r]).sum::<f64>() / k).sum::<f64>();
    println!("RESULT profile v={} op={} fill+collect_ms={:.4} cache_flush_ms={:.4} register_writes_ms={:.4} assembly_ms={:.4} layout_total_ms={:.4} calls={:.0} layout_ns_per_call={:.1}", v, name, tot / 1e6, fl / 1e6, rg / 1e6, asm / 1e6, (tot + fl + rg + asm) / 1e6, calls, (tot + fl + rg + asm) / calls); }
fn unhex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
fn flips() -> Vec<usize> { vec![0, N + 5, N + FK * (FA + 1) * N - 1, HTOFF + 100, HTOFF + 11 * (WL + HP) * N + 7, SIGLEN - 1] }

fn main() { unsafe {
    let args: Vec<String> = std::env::args().collect(); let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let emhz: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0); let mode = args.get(3).cloned().unwrap_or("std".to_string());
    let reps: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(25);
    let fwlen = 1usize << 20; let su = suite_of(mode.trim_start_matches("fw-")); let hf = su.f;
    OLD = args.get(5).map_or(false, |s| s == "old");
    let (engs, fva, fpa) = match setup(n, fwlen) { Some(x) => x, None => return };
    println!("RESULT sign3 atoms={} engine_mhz={} mode={} hash={} split_layout={} board={} reps={} ids={}", n, emhz, mode, su.name, if OLD { "old(ruty_sign.rs)" } else { "word" }, cfg!(target_arch = "aarch64"), reps,
        engs.iter().map(|e| format!("{:08x}", e.rd(ID))).collect::<Vec<String>>().join(" "));
    let standard = mode == "std" || mode == "fw" || mode == "tenants";
    if !standard { println!("RESULT NOTE NON-STANDARD: every hash call uses {} instead of SHAKE256. This is NOT FIPS 205 and NOT SLH-DSA; no external reference exists. Checked only for internal consistency.", su.name); }
    if mode == "hybrid" { println!("RESULT NOTE design choices (ours, not a standard): PRF and F (64-byte inputs, every WOTS+ chain step, FORS secret values and leaves) are MiniSHAKE128 (domain byte 0x01) on split atoms; H, T_len, T_k, H_msg and PRF_msg are TurboSHAKE256 (domain byte 0x1F) on wide atoms; the width is set per batch by register writes. n = 16 output bytes everywhere. NOT FIPS 205."); }
    if mode == "mini" { println!("RESULT NOTE design choices (ours, not a standard): every call (PRF, PRF_msg, H_msg, F, H, T_len, T_k) is MiniSHAKE128 with domain byte 0x01 and n = 16 output bytes, on split atoms (two Keccak-p[800] per atom); F and PRF (64 B) fit one 68-byte block, H (80 B), T_len (608 B), T_k (576 B), H_msg and PRF_msg take several blocks; with the word layout two XMSS trees share each node so no tree call is paired with a copy of itself; calls that are alone by dependency (PRF_msg, H_msg, T_k, odd WOTS+ chain-length groups, every verify step) are paired with a copy; chains run inside the atom, two at a time with equal step counts."); }
    let mut bad = 0usize;
    // ---------------- test keys and messages ----------------
    let mut rng = Rng(0x5EED_5167_2026_1126);
    let mut vecs: Vec<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> = Vec::new();
    vecs.push(((0..16u8).collect(), (16..32u8).collect(), (32..48u8).collect(), Vec::new()));
    { let a = rng.msg(16); let b = rng.msg(16); let c = rng.msg(16); let m = rng.msg(1024); vecs.push((a, b, c, m)); }
    { let a = rng.msg(16); let b = rng.msg(16); let c = rng.msg(16); vecs.push((a, b, c, b"RUTY-ATOM SLH-DSA-SHAKE-128f test".to_vec())); }
    // NIST ACVP-Server SLH-DSA-sigGen-FIPS205 internalProjection.json, SLH-DSA-SHAKE-128f, deterministic, internal interface (tg16 tc137): M' = the message itself
    let acvp_sk = unhex("1DB8D66744E6BCAA551A18B9CB6DD0D1B75CD984A6735CCE2B323621C53F42CAD541850011514487EF2212E435DC16F4FF63ABCDC770B395365F2D895346773D");
    vecs.push((acvp_sk[0..16].to_vec(), acvp_sk[16..32].to_vec(), acvp_sk[32..48].to_vec(), unhex("8F")));
    if mode.starts_with("fw") { fw_main(&engs, fva, fpa, fwlen, reps, &mut bad, &mode); println!("RESULT summary mode={} mismatches={} {}", mode, bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" }); return; }
    if mode == "tenants" { tenants_main(&engs, fva, fpa, fwlen, reps, &mut bad); println!("RESULT summary mode=tenants mismatches={} {}", bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" }); return; }
    for (vi, v) in vecs.iter().enumerate() { let (sks, skp, seed, msg) = (&v.0, &v.1, &v.2, &v.3);
        let internal = vi == 3; let vn = format!("{}", vi);
        let mut mp = if internal { Vec::new() } else { vec![0u8, 0u8] }; mp.extend_from_slice(msg);   // pure, empty context: M' = 0x00 || 0x00 || M; ACVP internal vector: M' = M
        let (root, _, e1) = atom_keygen(&engs, su, sks, seed); let swroot = sw_keygen(&su, sks, seed);
        println!("KEY v={} mode={} iface={} sk_seed={} sk_prf={} pk_seed={} pk_root={} msg_len={} msg={}", vi, mode, if internal { "internal" } else { "pure" }, hx(sks), hx(skp), hx(seed), hx(&root), msg.len(), hx(msg));
        let (sig, _, e2, top) = atom_sign(&engs, su, sks, skp, seed, &root, &mp);
        let t = now_ns(); let swsig = sw_sign(&su, sks, skp, seed, &swroot, &mp); let sw_sign_ms = (now_ns() - t) / 1e6;
        println!("SIG v={} mode={} len={} hex={}", vi, mode, sig.len(), hx(&sig));
        let (okv, _, e3) = atom_verify(&engs, su, seed, &root, &mp, &sig, 0);
        let t = now_ns(); let swok = sw_verify(&su, seed, &root, &mp, &sig); let sw_ver_ms = (now_ns() - t) / 1e6;
        let mut rej = 0; let mut swrej = 0; let fl = flips();
        for &p in &fl { let mut s2 = sig.clone(); s2[p] ^= 1 << (p % 8); if !atom_verify(&engs, su, seed, &root, &mp, &s2, 0).0 { rej += 1; } if !sw_verify(&su, seed, &root, &mp, &s2) { swrej += 1; } }
        let mut m2 = mp.clone(); if m2.len() > 2 { let l = m2.len() - 1; m2[l] ^= 0x01; } else { m2.push(0x00); }
        let rejm = !atom_verify(&engs, su, seed, &root, &m2, &sig, 0).0; let swrejm = !sw_verify(&su, seed, &root, &m2, &sig);
        let e = e1 + e2 + e3; let ok = root == swroot && sig == swsig && top && okv && swok && rej == fl.len() && swrej == fl.len() && rejm && swrejm && e == 0 && sig.len() == SIGLEN;
        if !ok { bad += 1; }
        println!("RESULT check v={} mode={} keygen_match_sw={} sign_match_sw={} top_root_is_pk_root={} atom_verify_good={} atom_reject_flipped={}/{} atom_reject_wrong_msg={} sw_verify_good={} sw_reject_flipped={}/{} sw_reject_wrong_msg={} errors={} {}",
            vi, mode, root == swroot, sig == swsig, top, okv, rej, fl.len(), rejm, swok, swrej, fl.len(), swrejm, e, if ok { "PASS" } else { "FAIL" });
        if !ok { println!("RESULT timing skipped v={}: signature not verified", vi); continue; }
        // ---------------- timing ----------------
        let (mut tk, mut ts, mut tv) = (Vec::new(), Vec::new(), Vec::new()); let (mut sk_s, mut sg_s, mut vf_s) = (Vec::new(), Vec::new(), Vec::new()); let mut tbad = 0;
        for _ in 0..reps {
            let t = now_ns(); let (r2, s1, ea) = atom_keygen(&engs, su, sks, seed); tk.push((now_ns() - t) / 1e6); if r2 != root || ea != 0 { tbad += 1; } sk_s.push(s1);
            let t = now_ns(); let (s2, s1, eb, _) = atom_sign(&engs, su, sks, skp, seed, &root, &mp); ts.push((now_ns() - t) / 1e6); if s2 != sig || eb != 0 { tbad += 1; } sg_s.push(s1);
            let t = now_ns(); let (o2, s1, ec) = atom_verify(&engs, su, seed, &root, &mp, &sig, 0); tv.push((now_ns() - t) / 1e6); if !o2 || ec != 0 { tbad += 1; } vf_s.push(s1); }
        // verify spread over fewer atoms (latency-bound: fewer, larger batches)
        let mut vp: Vec<(usize, Vec<f64>)> = Vec::new();
        for &pp in [1usize, 2].iter() { if pp >= n { continue; } let mut tt = Vec::new();
            for _ in 0..reps { let t = now_ns(); let (o2, _, ec) = atom_verify(&engs, su, seed, &root, &mp, &sig, pp); tt.push((now_ns() - t) / 1e6); if !o2 || ec != 0 { tbad += 1; } } vp.push((pp, tt)); }
        bad += tbad;
        println!("RESULT time v={} mode={} op=keygen atoms={} mhz={} {} wrong={}", vi, mode, n, emhz, stats(&tk), tbad);
        println!("RESULT time v={} mode={} op=sign atoms={} mhz={} {} wrong={} msg_len={}", vi, mode, n, emhz, stats(&ts), tbad, msg.len());
        println!("RESULT time v={} mode={} op=verify atoms={} mhz={} {} wrong={}", vi, mode, n, emhz, stats(&tv), tbad);
        for &(pp, ref tt) in vp.iter() { println!("RESULT time v={} mode={} op=verify_on_{}_of_{}_atoms mhz={} {} wrong={}", vi, mode, pp, n, emhz, stats(tt), tbad); }
        prof("sign", &vn, &sg_s); prof("verify", &vn, &vf_s);
        bd("keygen", &vn, &sk_s, &[C_TREE]); bd("sign", &vn, &sg_s, &[C_MSG, C_FORS, C_TREE, C_WOTS]); bd("verify", &vn, &vf_s, &[C_MSG, C_VFORS, C_VHT]);
        println!("RESULT software_a53 v={} mode={} sign_ms={:.1} verify_ms={:.1} (one run, portable Rust on one A53 core; the program's own reference)", vi, mode, sw_sign_ms, sw_ver_ms); }
    println!("RESULT summary mode={} mismatches={} {}", mode, bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" });
} }

// ================= firmware authentication: SHAKE256 of a 1 MB image + HashSLH-DSA-SHAKE-128f, all hashing on the atoms =================
unsafe fn fw_main(engs: &[Eng], fva: *mut u8, fpa: u64, fwlen: usize, reps: usize, bad: &mut usize, mode: &str) {
    FWTURBO = mode != "fw"; let hf = if FWTURBO { hf_of("turbo") } else { hf_of("std") }; let su = suite_of(mode.trim_start_matches("fw-")); let mut rng = Rng(0xF1F1_2026_1126_0001);
    let oid: [u8; 11] = if FWTURBO { [0u8; 11] } else { OID_SHAKE256 };
    let image = rng.msg(fwlen); std::ptr::copy_nonoverlapping(image.as_ptr(), fva, fwlen);
    let sks = rng.msg(16); let skp = rng.msg(16); let seed = rng.msg(16);
    let mprime = |dig: &[u8]| { let mut m = vec![1u8, 0u8]; m.extend_from_slice(&oid); m.extend_from_slice(dig); m };   // 0x01 || |ctx| = 0 || OID || PH(M)
    let (root, _, e0) = atom_keygen(engs, su, &sks, &seed); let swroot = sw_keygen(&su, &sks, &seed);
    let swdig = xofd(&hf.g, hf.dom, &image, 64);
    println!("KEY v=fw mode={} sk_seed={} sk_prf={} pk_seed={} pk_root={} image_rng_seed=0xF1F120261126_0001 image_len={} image_{}_512={}", mode, hx(&sks), hx(&skp), hx(&seed), hx(&root), fwlen, hf.name, hx(&swdig));
    // signer: hash on an atom (no copy: the atom reads the image where it lies), then sign on the atoms
    let (dig, _, e1) = atom_fw_hash(engs, fva, fpa, fwlen); let mp = mprime(&dig);
    let (sig, _, e2, top) = atom_sign(engs, su, &sks, &skp, &seed, &root, &mp); let swsig = sw_sign(&su, &sks, &skp, &seed, &swroot, &mprime(&swdig));
    println!("SIG v=fw mode={} len={} hex={}", mode, sig.len(), hx(&sig));
    // verifier
    let (dig2, _, e3) = atom_fw_hash(engs, fva, fpa, fwlen); let (okv, _, e4) = atom_verify(engs, su, &seed, &root, &mprime(&dig2), &sig, 0);
    // tampered image (one bit) and tampered signature must be rejected
    *fva.add(123_457) ^= 0x10; let (dig3, _, e5) = atom_fw_hash(engs, fva, fpa, fwlen); let rej_img = !atom_verify(engs, su, &seed, &root, &mprime(&dig3), &sig, 0).0; *fva.add(123_457) ^= 0x10;
    let mut s2 = sig.clone(); s2[HTOFF + 333] ^= 4; let rej_sig = !atom_verify(engs, su, &seed, &root, &mprime(&dig2), &s2, 0).0;
    let swok = sw_verify(&su, &seed, &root, &mprime(&swdig), &sig);
    let e = e0 + e1 + e2 + e3 + e4 + e5;
    let ok = root == swroot && dig == swdig && dig2 == swdig && dig3 != swdig && sig == swsig && top && okv && rej_img && rej_sig && swok && e == 0;
    if !ok { *bad += 1; }
    println!("RESULT check v=fw mode={} prehash={} keygen_match_sw={} digest_match_sw={} sign_match_sw={} atom_verify_good={} atom_reject_tampered_image={} atom_reject_flipped_sig={} sw_verify_good={} errors={} {}",
        mode, hf.name, root == swroot, dig == swdig, sig == swsig, okv, rej_img, rej_sig, swok, e, if ok { "PASS" } else { "FAIL" });
    if !ok { println!("RESULT timing skipped v=fw: not verified"); return; }
    let (mut sh, mut ss, mut st_, mut vh, mut vv, mut vt) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()); let mut tbad = 0;
    for _ in 0..reps {
        let t0 = now_ns(); let (d, _, ea) = atom_fw_hash(engs, fva, fpa, fwlen); let t1 = now_ns(); let (s, _, eb, _) = atom_sign(engs, su, &sks, &skp, &seed, &root, &mprime(&d)); let t2 = now_ns();
        sh.push((t1 - t0) / 1e6); ss.push((t2 - t1) / 1e6); st_.push((t2 - t0) / 1e6); if s != sig || ea + eb != 0 { tbad += 1; }
        let t0 = now_ns(); let (d, _, ea) = atom_fw_hash(engs, fva, fpa, fwlen); let t1 = now_ns(); let (o, _, eb) = atom_verify(engs, su, &seed, &root, &mprime(&d), &sig, 0); let t2 = now_ns();
        vh.push((t1 - t0) / 1e6); vv.push((t2 - t1) / 1e6); vt.push((t2 - t0) / 1e6); if !o || ea + eb != 0 { tbad += 1; } }
    *bad += tbad;
    println!("RESULT time v=fw mode={} op=signer_total {} wrong={}", mode, stats(&st_), tbad); println!("RESULT time v=fw mode={} op=signer_hash_1MB {}", mode, stats(&sh)); println!("RESULT time v=fw mode={} op=signer_sign {}", mode, stats(&ss));
    println!("RESULT time v=fw mode={} op=verifier_total {} wrong={}", mode, stats(&vt), tbad); println!("RESULT time v=fw mode={} op=verifier_hash_1MB {}", mode, stats(&vh)); println!("RESULT time v=fw mode={} op=verifier_verify {}", mode, stats(&vv)); }

// ================= two tenants running FULL applications at once (4-atom image) =================
// Tenant S: SLH-DSA-SHAKE-128f signing of the 1 kB test message (vector 1) on atoms 0..k. Tenant T: firmware authentication verify
// (SHAKE256 of the 1 MB image on an atom + SLH-DSA verify; every 4th run with a flipped signature, which must be rejected) on atoms k..4.
// Each tenant has its own scheduler on its own A53 core (two threads); they share only the memory port and the processor's caches.
struct Ctx { s_sks: Vec<u8>, s_skp: Vec<u8>, s_seed: Vec<u8>, s_root: Vec<u8>, mp: Vec<u8>, s_ref: Vec<u8>,
             t_seed: Vec<u8>, t_root: Vec<u8>, t_sig: Vec<u8>, t_bad: Vec<u8>, dig: Vec<u8>, fva: usize, fpa: u64, fwlen: usize, engp: usize }
unsafe fn engs_of(c: &Ctx, a: usize, b: usize) -> &'static [Eng] { std::slice::from_raw_parts((c.engp as *const Eng).add(a), b - a) }
unsafe fn op_s(c: &Ctx, a: usize, b: usize) -> (f64, bool) { let e = engs_of(c, a, b); let t = now_ns();
    let (s, _, er, _) = atom_sign(e, suite_of("std"), &c.s_sks, &c.s_skp, &c.s_seed, &c.s_root, &c.mp); ((now_ns() - t) / 1e6, s == c.s_ref && er == 0) }
unsafe fn op_t(c: &Ctx, a: usize, b: usize, i: usize) -> (f64, bool) { let e = engs_of(c, a, b); let t = now_ns();
    let (d, _, e1) = atom_fw_hash(e, c.fva as *mut u8, c.fpa, c.fwlen); let mut m = vec![1u8, 0u8]; m.extend_from_slice(&OID_SHAKE256); m.extend_from_slice(&d);
    let flip = i % 4 == 3; let (ok, _, e2) = atom_verify(e, suite_of("std"), &c.t_seed, &c.t_root, &m, if flip { &c.t_bad } else { &c.t_sig }, 0);
    ((now_ns() - t) / 1e6, ok != flip && e1 + e2 == 0 && d == c.dig) }
fn median(v: &[f64]) -> f64 { let mut s = v.to_vec(); s.sort_by(|a, b| a.partial_cmp(b).unwrap()); s[s.len() / 2] }
unsafe fn tenants_main(engs: &[Eng], fva: *mut u8, fpa: u64, fwlen: usize, reps: usize, bad: &mut usize) {
    use std::sync::atomic::{AtomicBool, Ordering}; use std::sync::Arc;
    if engs.len() != 4 { println!("RESULT error=tenants_needs_the_4_atom_image"); *bad += 1; return; }
    FWTURBO = false; let su = suite_of("std");
    let mut rng = Rng(0x5EED_5167_2026_1126); let _ = rng.msg(0); let s_sks = rng.msg(16); let s_skp = rng.msg(16); let s_seed = rng.msg(16); let s_msg = rng.msg(1024);
    let mut mp = vec![0u8, 0u8]; mp.extend_from_slice(&s_msg);
    let s_root = sw_keygen(&su, &s_sks, &s_seed); let s_ref = sw_sign(&su, &s_sks, &s_skp, &s_seed, &s_root, &mp);
    let mut r2 = Rng(0xF1F1_2026_1126_0001); let image = r2.msg(fwlen); std::ptr::copy_nonoverlapping(image.as_ptr(), fva, fwlen);
    let t_sks = r2.msg(16); let t_skp = r2.msg(16); let t_seed = r2.msg(16);
    let t_root = sw_keygen(&su, &t_sks, &t_seed); let dig = xofd(&hf_of("std").g, 0x1F, &image, 64);
    let mut tm = vec![1u8, 0u8]; tm.extend_from_slice(&OID_SHAKE256); tm.extend_from_slice(&dig);
    let t_sig = sw_sign(&su, &t_sks, &t_skp, &t_seed, &t_root, &tm); let mut t_bad = t_sig.clone(); t_bad[HTOFF + 333] ^= 4;
    println!("KEY v=1 mode=std iface=pure sk_seed={} sk_prf={} pk_seed={} pk_root={} msg_len={} msg={}", hx(&s_sks), hx(&s_skp), hx(&s_seed), hx(&s_root), s_msg.len(), hx(&s_msg));
    println!("SIG v=1 mode=std len={} hex={}", s_ref.len(), hx(&s_ref));
    println!("KEY v=fw mode=fw sk_seed={} sk_prf={} pk_seed={} pk_root={} image_rng_seed=0xF1F120261126_0001 image_len={} image_shake256_512={}", hx(&t_sks), hx(&t_skp), hx(&t_seed), hx(&t_root), fwlen, hx(&dig));
    println!("SIG v=fw mode=fw len={} hex={}", t_sig.len(), hx(&t_sig));
    let ctx: &'static Ctx = Box::leak(Box::new(Ctx { s_sks: s_sks, s_skp: s_skp, s_seed: s_seed, s_root: s_root, mp: mp, s_ref: s_ref, t_seed: t_seed, t_root: t_root,
        t_sig: t_sig, t_bad: t_bad, dig: dig, fva: fva as usize, fpa: fpa, fwlen: fwlen, engp: engs.as_ptr() as usize }));
    let caddr = ctx as *const Ctx as usize;
    for &k in [1usize, 2, 3].iter() {
        let (mut sa, mut ta, mut ss, mut tt) = (Vec::new(), Vec::new(), Vec::new(), Vec::new()); let mut wrong = 0usize;
        for _ in 0..reps { let (x, ok) = op_s(ctx, 0, k); sa.push(x); if !ok { wrong += 1; } }
        for i in 0..reps { let (x, ok) = op_t(ctx, k, 4, i); ta.push(x); if !ok { wrong += 1; } }
        let mut bg = [(0usize, 0usize); 2];
        for which in 0..2 {           // 0: measure S while T runs; 1: measure T while S runs
            let stop = Arc::new(AtomicBool::new(false)); let st2 = stop.clone();
            let h = std::thread::spawn(move || { let c = unsafe { &*(caddr as *const Ctx) }; let (mut n, mut b) = (0usize, 0usize);
                while !st2.load(Ordering::Relaxed) { let ok = unsafe { if which == 0 { op_t(c, k, 4, n).1 } else { op_s(c, 0, k).1 } }; if !ok { b += 1; } n += 1; } (n, b) });
            let t0 = now_ns(); while now_ns() - t0 < 3e6 {}            // let the background tenant start
            for i in 0..reps { if which == 0 { let (x, ok) = op_s(ctx, 0, k); ss.push(x); if !ok { wrong += 1; } } else { let (x, ok) = op_t(ctx, k, 4, i); tt.push(x); if !ok { wrong += 1; } } }
            stop.store(true, Ordering::Relaxed); bg[which] = h.join().unwrap(); wrong += bg[which].1; }
        *bad += wrong;
        let tg: Vec<f64> = ta.iter().enumerate().filter(|&(i, _)| i % 4 != 3).map(|(_, x)| *x).collect(); let tgs: Vec<f64> = tt.iter().enumerate().filter(|&(i, _)| i % 4 != 3).map(|(_, x)| *x).collect();
        println!("RESULT tenants split=S{}+T{} S_sign_alone_ms={:.4} S_sign_shared_ms={:.4} S_ratio={:.4} T_fwverify_alone_ms={:.4} T_fwverify_shared_ms={:.4} T_ratio={:.4} bg_runs_T={} bg_runs_S={} n={} wrong={} {}",
            k, 4 - k, median(&sa), median(&ss), median(&ss) / median(&sa), median(&tg), median(&tgs), median(&tgs) / median(&tg), bg[0].0, bg[1].0, reps, wrong, if wrong == 0 { "ALL_CORRECT" } else { "FAIL" });
        println!("RESULT tenants split=S{}+T{} detail S_alone {} | S_shared {} | T_alone(accept runs) {} | T_shared(accept runs) {}", k, 4 - k, stats(&sa), stats(&ss), stats(&tg), stats(&tgs)); } }
