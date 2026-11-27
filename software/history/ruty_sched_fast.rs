// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy

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
