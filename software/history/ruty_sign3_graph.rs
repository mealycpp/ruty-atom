// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
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
    if st.su.f.dual { (0..WL).filter(|&c| dd[c] as usize == g && dd[c] != 15).collect() }
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
        Nk::FwH => { let s = hf_of("std"); Job { g: s.g, dom: s.dom, dual: false, len: st.fwlen, olen: 64, cend: 0, cnt: 1, pc: 1, inp: st.fwpa } } } }

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
    let mut pv = fr; let ng = if st.su.f.dual { 15 } else { st.np };
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
