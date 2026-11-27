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

// ================= ruty_dfg: a job graph scheduled on the atoms =================
// A node is one batch of equal-shaped hash jobs. An edge says a node needs the digests of another node.
// The graph here: 22 XMSS trees of SLH-DSA-128f in SHAKE256 (6 dependent nodes per tree; the chain node runs inside the atom),
// B tag nodes (2400 MiniSHAKE128 tags each, the atom is split into two 800-bit cores) and F firmware nodes (1 MB TurboSHAKE128 each).
// For every node the scheduler sets the atom it runs on to the width, round count and operation the node needs (register writes only).
// Policies:  0 serial      one tenant after another, each on all atoms
//            1 graph       any ready node on any free atom, tenants alternate
//            2 graph+rule  stream nodes may load the port up to its measured limit (tags 1 unit, firmware 2 units, budget 2); the rest goes to tree nodes
//   sudo ./ruty_dfg <atoms in the image> [engine MHz]
static mut ZC: bool = false; static mut TAG_VA: *mut u8 = std::ptr::null_mut(); static mut FW_VA: *mut u8 = std::ptr::null_mut(); static mut TAG_PA: u64 = 0; static mut FW_PA: u64 = 0;
const TAGSTR: usize = 241664;          // tag nodes sit 59 pages apart in the tenant buffer
unsafe fn win(e: &Eng, pa: u64, len: usize) { let lo = if pa < e.pa { pa } else { e.pa }; let h1 = pa + len as u64; let h2 = e.pa + PAGE as u64; let hi = if h1 > h2 { h1 } else { h2 };
    e.wr(WBASE, (lo >> 12) as u32); e.wr(WLIMIT, ((hi + 4095) >> 12) as u32); }
const TAGB: usize = 2400; const TAGIN: usize = TAGB / 2 * 200; const TAGOUT: usize = TAGB / 2 * 32; const FWLEN: usize = 1 << 20;
#[derive(Clone, Copy, PartialEq)] enum Kind { Tree, Tags, Stream }
struct Node { kind: Kind, a: usize, b: usize, succ: Option<usize>, units: u32 }      // Tree: a = tree, b = stage.  Tags, Stream: a = index
struct Res { ns: f64, t_tree: f64, t_tags: f64, t_fw: f64, util: f64, bad: usize, batches: usize }
fn take_tree(rt: &mut Vec<usize>, nodes: &Vec<Node>) -> Option<usize> { if rt.is_empty() { return None; } let mut bi = 0;
    for i in 1..rt.len() { if nodes[rt[i]].b > nodes[rt[bi]].b { bi = i; } } Some(rt.swap_remove(bi)) }
fn take_stream(rs: &mut Vec<usize>, nodes: &Vec<Node>, budget: u32) -> Option<usize> { for i in 0..rs.len() { if nodes[rs[i]].units <= budget { return Some(rs.remove(i)); } } None }
unsafe fn run_dfg(engs: &[Eng], policy: u8, tb: usize, fwn: usize, seed: &[u8], sks: &[u8], tag_in: &[u8], fw_in: &[u8],
                  want: &[Vec<u8>], tag_ref: &[Vec<u8>], fw_ref: &[Vec<u8>]) -> Res {
    let n = engs.len(); let g = &ALGS[1]; let mut trees = new_trees(); let mut nodes: Vec<Node> = Vec::new();
    for t in 0..NTREE { for s in 0..6 { nodes.push(Node { kind: Kind::Tree, a: t, b: s, succ: if s < 5 { Some(t * 6 + s + 1) } else { None }, units: 0 }); } }
    for k in 0..tb { nodes.push(Node { kind: Kind::Tags, a: k, b: 0, succ: None, units: 1 }); }
    for k in 0..fwn { nodes.push(Node { kind: Kind::Stream, a: k, b: 0, succ: None, units: 2 }); }
    let mut rt: Vec<usize> = (0..NTREE).map(|t| t * 6).collect(); let mut rs: Vec<usize> = (NTREE * 6..nodes.len()).collect();
    let mut tag_out = vec![0u8; tb * TAGOUT + 8]; let mut fw_out = vec![0u8; fwn * 32 + 8];
    let mut at: Vec<Option<(usize, usize, usize)>> = vec![None; n]; let mut since = vec![0.0f64; n]; let mut busy = 0.0f64;
    let (mut left, mut trees_left, mut tags_left, mut fw_left) = (nodes.len(), NTREE * 6, tb, fwn);
    let (mut tt, mut tg, mut tf) = (0.0f64, 0.0f64, 0.0f64); let (mut units_run, mut flip, mut ok, mut spins, mut batches) = (0u32, false, true, 0u64, 0usize);
    let t0 = now_ns();
    while left > 0 { spins += 1; if spins > 2_000_000_000 { ok = false; break; }
        for ai in 0..n { let e = &engs[ai];
            if let Some((ni, cnt, pc)) = at[ai] { if let Some(s) = e.finished() { ok &= s & 0x72 == 2; let now = now_ns(); busy += now - since[ai]; let ob = e.buf(1).0;
                match nodes[ni].kind {
                    Kind::Tree => { from_device(ob, pc * 16); let t = &mut trees[nodes[ni].a];
                        let dst = match t.stage { 0 => t.sk.as_mut_ptr(), 1 => t.ends.as_mut_ptr(), _ => t.lvl.as_mut_ptr() };
                        for k in 0..cnt { rdg(ob, false, k, dst.add(k * 16)); }
                        if t.stage >= 2 { t.nl = cnt; } t.stage += 1; trees_left -= 1; if trees_left == 0 { tt = now - t0; } }
                    Kind::Tags => { from_device(ob, TAGOUT); std::ptr::copy_nonoverlapping(ob, tag_out.as_mut_ptr().add(nodes[ni].a * TAGOUT), TAGOUT); tags_left -= 1; if tags_left == 0 { tg = now - t0; } }
                    Kind::Stream => { from_device(ob, 32); std::ptr::copy_nonoverlapping(ob, fw_out.as_mut_ptr().add(nodes[ni].a * 32), 32); fw_left -= 1; if fw_left == 0 { tf = now - t0; } } }
                units_run -= nodes[ni].units; if let Some(sx) = nodes[ni].succ { rt.push(sx); } left -= 1; at[ai] = None; } }
            if at[ai].is_none() && left > 0 {
                let pick = match policy {
                    0 => if trees_left > 0 { take_tree(&mut rt, &nodes) } else { take_stream(&mut rs, &nodes, u32::MAX) },
                    1 => { flip = !flip; if flip { let x = take_stream(&mut rs, &nodes, u32::MAX); if x.is_some() { x } else { take_tree(&mut rt, &nodes) } }
                           else { let x = take_tree(&mut rt, &nodes); if x.is_some() { x } else { take_stream(&mut rs, &nodes, u32::MAX) } } },
                    _ => { let x = take_stream(&mut rs, &nodes, 2u32.saturating_sub(units_run)); if x.is_some() { x } else { take_tree(&mut rt, &nodes) } } };
                if let Some(ni) = pick { let a = nodes[ni].a; let mut inp = e.buf(0).1;
                    let (cnt, pc, nj) = match nodes[ni].kind {
                        Kind::Tree => { let (cnt, pc, ln, chain) = fill(e, &trees[a], false, seed, sks);
                            e.wr(LEN, ln as u32 | (0x1F << 24)); e.wr(OLEN, 16); e.wr(ALG, g.first | (g.last << 8) | ((g.rate as u32 / 8) << 24));
                            if chain { e.wr(0x44, 1 | ((47 / 8) << 8) | ((47 % 8) << 16) | ((48 / 8) << 20)); e.wr(0x48, 15); } else { e.wr(0x44, 0); } (cnt, pc, pc) }
                        Kind::Tags => { if ZC { let sp = TAG_PA + (a * TAGSTR) as u64; to_device(TAG_VA.add(a * TAGSTR), TAGIN); win(e, sp, TAGIN); inp = sp; } else { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(tag_in.as_ptr().add(a * TAGIN), p, TAGIN); to_device(p, TAGIN); } let m = &ALGS[4];
                            e.wr(LEN, 100 | (0x01 << 24)); e.wr(OLEN, 16); e.wr(ALG, m.first | (m.last << 8) | (1 << 16) | ((m.rate as u32 / 4) << 24)); e.wr(0x44, 0); (TAGB, TAGB, TAGB / 2) }
                        Kind::Stream => { if ZC { let sp = FW_PA + (a * FWLEN) as u64; to_device(FW_VA.add(a * FWLEN), FWLEN); win(e, sp, FWLEN); inp = sp; } else { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(fw_in.as_ptr().add(a * FWLEN), p, FWLEN); to_device(p, FWLEN); } let m = &ALGS[2];
                            e.wr(LEN, FWLEN as u32 | (0x1F << 24)); e.wr(OLEN, 32); e.wr(ALG, m.first | (m.last << 8) | ((m.rate as u32 / 8) << 24)); e.wr(0x44, 0); (1, 1, 1) } };
                    e.start(inp, e.buf(1).1, nj); since[ai] = now_ns(); units_run += nodes[ni].units; at[ai] = Some((ni, cnt, pc)); batches += 1; } } } }
    let dt = now_ns() - t0; for e in engs.iter() { e.wr(0x44, 0); }
    let mut bad = (!ok) as usize;
    for t in 0..NTREE { if trees[t].stage != 6 || trees[t].lvl[0..16] != want[t][..] { bad += 1; } }
    for k in 0..tb * TAGB { let (bt, kk) = (k / TAGB, k % TAGB); for i in 0..16 { if tag_out[bt * TAGOUT + (kk / 2) * 32 + (i / 4) * 8 + (kk % 2) * 4 + i % 4] != tag_ref[k][i] { bad += 1; break; } } }
    for k in 0..fwn { if fw_out[k * 32..k * 32 + 32] != fw_ref[k][..] { bad += 1; } }
    Res { ns: dt, t_tree: tt, t_tags: tg, t_fw: tf, util: busy / (n as f64 * dt), bad, batches } }
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
    println!("RESULT dfg atoms={} engine_mhz={} ids={}", n, emhz, engs.iter().map(|e| format!("{:08x}", e.rd(ID))).collect::<Vec<String>>().join(" "));
    const TBMAX: usize = 16; const FWMAX: usize = 4;
    let seed: Vec<u8> = (0..16u8).collect(); let sks: Vec<u8> = (100..116u8).collect();
    let want: Vec<Vec<u8>> = new_trees().iter().map(|t| ref_root(&ALGS[1], 0x1F, &seed, &sks, t.layer, t.tree)).collect();
    let mut rng = Rng(0x5EED_0DF6_2026_1126); let tags = rng.msg(TBMAX * TAGB * 100); let fw_in = rng.msg(FWMAX * FWLEN);
    let tag_ref: Vec<Vec<u8>> = (0..TBMAX * TAGB).map(|j| xofd(&ALGS[4], 0x01, &tags[j * 100..j * 100 + 100], 16)).collect();
    let fw_ref: Vec<Vec<u8>> = (0..FWMAX).map(|k| xofd(&ALGS[2], 0x1F, &fw_in[k * FWLEN..(k + 1) * FWLEN], 32)).collect();
    let mut tag_in = vec![0u8; TBMAX * TAGIN];           // tags laid out once in the order the split atom reads them
    for k in 0..TBMAX * TAGB { let (bt, kk) = (k / TAGB, k % TAGB); for i in 0..100 { tag_in[bt * TAGIN + (kk / 2) * 200 + (i / 4) * 8 + (kk % 2) * 4 + i % 4] = tags[k * 100 + i]; } }
    let names = ["serial", "graph", "graph+rule"]; let mut allbad = 0usize;
    let mut big = [(std::ptr::null_mut::<u8>(), 0u64); 2];
    for bi in 0..2 { let len = 4usize << 20;
        let mut a = HeapAlloc { len: len as u64, fd: 0, fd_flags: 2 | 0x80000, heap_flags: 0 };
        if ioctl(hf, 0xC018_4800, &mut a as *mut HeapAlloc) != 0 { println!("RESULT error=no_big_buffer"); return; }
        let va = mmap(std::ptr::null_mut(), len, 3, 1, a.fd as c_int, 0) as *mut u8; if va as isize == -1 { println!("RESULT error=cannot_map_big"); return; }
        let mut k = 0; while k < len { *va.add(k) = 0; k += 4096; }
        let mut pf = [0u64; 2];
        for (j, off) in [0usize, len - 4096].iter().enumerate() { let mut e = [0u8; 8]; pm.seek(SeekFrom::Start(((va as u64 + *off as u64) / 4096) * 8)).unwrap(); pm.read_exact(&mut e).unwrap(); pf[j] = u64::from_le_bytes(e) & ((1 << 55) - 1); }
        let pa = pf[0] * 4096; if pf[1] - pf[0] != (len as u64 / 4096) - 1 || pa + len as u64 > LIMIT { println!("RESULT error=big_buffer_not_usable pa={:#x}", pa); return; }
        big[bi] = (va, pa); }
    TAG_VA = big[0].0; TAG_PA = big[0].1; FW_VA = big[1].0; FW_PA = big[1].1;
    for b in 0..TBMAX { std::ptr::copy_nonoverlapping(tag_in.as_ptr().add(b * TAGIN), TAG_VA.add(b * TAGSTR), TAGIN); }
    std::ptr::copy_nonoverlapping(fw_in.as_ptr(), FW_VA, FWMAX * FWLEN);
    println!("RESULT dfg tenant_buffers tags_pa={:#x} fw_pa={:#x} atom_pa={}", TAG_PA, FW_PA, engs.iter().map(|e| format!("{:#x}", e.pa)).collect::<Vec<String>>().join(" "));
    for zc in [false, true].iter() { ZC = *zc; println!("RESULT dfg mode no_copy={}", *zc);
    for &(tb, fwn) in [(0usize, 0usize), (4, 1), (8, 2), (16, 4), (16, 0), (0, 4)].iter() { let mut ms = [0.0f64; 3];
        for p in 0..3u8 { let mut best: Option<Res> = None; let mut bad = 0; let mut all = Vec::new();
            for _ in 0..7 { let r = run_dfg(&engs, p, tb, fwn, &seed, &sks, &tag_in, &fw_in, &want, &tag_ref, &fw_ref); bad += r.bad; all.push(r.ns / 1e6);
                if r.bad == 0 && best.as_ref().map_or(true, |b| r.ns < b.ns) { best = Some(r); } }
            allbad += bad; all.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if let Some(b) = best { ms[p as usize] = b.ns / 1e6;
                println!("RESULT dfg mix=trees22+tags{}x{}+fw{}x1MB policy={} atoms={} ms={:.3} median_ms={:.3} trees_done_ms={:.3} tags_done_ms={:.3} fw_done_ms={:.3} atom_util={:.3} nodes={} mismatches={}",
                    tb, TAGB, fwn, names[p as usize], n, b.ns / 1e6, all[all.len() / 2], b.t_tree / 1e6, b.t_tags / 1e6, b.t_fw / 1e6, b.util, b.batches, bad); }
            else { println!("RESULT dfg mix=trees22+tags{}x{}+fw{}x1MB policy={} atoms={} FAILED mismatches={}", tb, TAGB, fwn, names[p as usize], n, bad); } }
        if ms[0] > 0.0 { println!("RESULT dfg gain mix=tags{}+fw{} graph_vs_serial={:.4} rule_vs_serial={:.4} rule_vs_graph={:.4}", tb, fwn, ms[1] / ms[0], ms[2] / ms[0], ms[2] / ms[1]); } }
    }
    println!("RESULT dfg summary mismatches={} {}", allbad, if allbad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
