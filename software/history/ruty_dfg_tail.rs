// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy

// ================= ruty_dfg: a job graph scheduled on the atoms =================
// A node is one batch of equal-shaped hash jobs. An edge says a node needs the digests of another node.
// The graph here: 22 XMSS trees of SLH-DSA-128f in SHAKE256 (6 dependent nodes per tree; the chain node runs inside the atom),
// B tag nodes (2400 MiniSHAKE128 tags each, the atom is split into two 800-bit cores) and F firmware nodes (1 MB TurboSHAKE128 each).
// For every node the scheduler sets the atom it runs on to the width, round count and operation the node needs (register writes only).
// Policies:  0 serial      one tenant after another, each on all atoms
//            1 graph       any ready node on any free atom, tenants alternate
//            2 graph+rule  stream nodes may load the port up to its measured limit (tags 1 unit, firmware 2 units, budget 2); the rest goes to tree nodes
//   sudo ./ruty_dfg <atoms in the image> [engine MHz]
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
                if let Some(ni) = pick { let a = nodes[ni].a;
                    let (cnt, pc, nj) = match nodes[ni].kind {
                        Kind::Tree => { let (cnt, pc, ln, chain) = fill(e, &trees[a], false, seed, sks);
                            e.wr(LEN, ln as u32 | (0x1F << 24)); e.wr(OLEN, 16); e.wr(ALG, g.first | (g.last << 8) | ((g.rate as u32 / 8) << 24));
                            if chain { e.wr(0x44, 1 | ((47 / 8) << 8) | ((47 % 8) << 16) | ((48 / 8) << 20)); e.wr(0x48, 15); } else { e.wr(0x44, 0); } (cnt, pc, pc) }
                        Kind::Tags => { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(tag_in.as_ptr().add(a * TAGIN), p, TAGIN); to_device(p, TAGIN); let m = &ALGS[4];
                            e.wr(LEN, 100 | (0x01 << 24)); e.wr(OLEN, 16); e.wr(ALG, m.first | (m.last << 8) | (1 << 16) | ((m.rate as u32 / 4) << 24)); e.wr(0x44, 0); (TAGB, TAGB, TAGB / 2) }
                        Kind::Stream => { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(fw_in.as_ptr().add(a * FWLEN), p, FWLEN); to_device(p, FWLEN); let m = &ALGS[2];
                            e.wr(LEN, FWLEN as u32 | (0x1F << 24)); e.wr(OLEN, 32); e.wr(ALG, m.first | (m.last << 8) | ((m.rate as u32 / 8) << 24)); e.wr(0x44, 0); (1, 1, 1) } };
                    e.start(e.buf(0).1, e.buf(1).1, nj); since[ai] = now_ns(); units_run += nodes[ni].units; at[ai] = Some((ni, cnt, pc)); batches += 1; } } } }
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
    println!("RESULT dfg summary mismatches={} {}", allbad, if allbad == 0 { "ALL_MATCH" } else { "FAIL" });
} }
