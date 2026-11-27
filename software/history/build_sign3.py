#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# build_sign3.py: makes ruty_sign3.rs (NEW) from ruty_sign.rs (left unchanged) and graph3.rs (the new job-graph section).
# usage: python build_sign3.py ruty_sign.rs graph3.rs ruty_sign3.rs
import sys, re
SRC, GR, DST = sys.argv[1], sys.argv[2], sys.argv[3]
L = open(SRC, newline='').read().replace('\r\n', '\n').split('\n')
head = '\n'.join(L[:119]) + '\n'; b = '\n'.join(L[119:397]) + '\n'; m = '\n'.join(L[397:])
# ---- suite (f: PRF and F; h: H, T_l, H_msg, PRF_msg)
x = '''    _ => Hf { name: "shake256", g: ALGS[1], dom: 0x1F, dual: false } } }'''
assert b.count(x) == 1
b = b.replace(x, x + '''
// The pair of hashes used: f for the one-block calls (PRF and F: 64-byte inputs), h for the rest (H, T_len, T_k, H_msg, PRF_msg).
// std: both SHAKE256 (FIPS 205). turbo, mini: both the same non-standard function. hybrid (NON-STANDARD, our choice): f = MiniSHAKE128 on
// split atoms, h = TurboSHAKE256 on wide atoms; each atom is set to the width of the call it runs, per batch.
#[derive(Clone, Copy)] struct Suite { name: &'static str, f: Hf, h: Hf }
fn suite_of(mode: &str) -> Suite { match mode {
    "turbo" => Suite { name: "turboshake256", f: hf_of("turbo"), h: hf_of("turbo") },
    "mini" => Suite { name: "minishake128", f: hf_of("mini"), h: hf_of("mini") },
    "hybrid" => Suite { name: "hybrid(minishake128:PRF,F|turboshake256:H,T_l,H_msg,PRF_msg)", f: hf_of("mini"), h: hf_of("turbo") },
    _ => Suite { name: "shake256", f: hf_of("std"), h: hf_of("std") } } }''')
# ---- software reference with roles
sw_old = b[b.index('fn hs(hf: &Hf'):b.index('// ================= the job graph')]
sw = sw_old
for a, c in [('fn sw_chain(hf: &Hf,', 'fn sw_chain(su: &Suite,'), ('v = hs(hf, &[seed, &adrs(layer, tree, WOTS_HASH', 'v = hs(&su.f, &[seed, &adrs(layer, tree, WOTS_HASH'),
    ('fn sw_wsk(hf: &Hf,', 'fn sw_wsk(su: &Suite,'), ('{ hs(hf, &[seed, &adrs(layer, tree, WOTS_PRF', '{ hs(&su.f, &[seed, &adrs(layer, tree, WOTS_PRF'),
    ('fn sw_xnode(hf: &Hf,', 'fn sw_xnode(su: &Suite,'), ('fn sw_fnode(hf: &Hf,', 'fn sw_fnode(su: &Suite,'),
    ('let sk = sw_wsk(hf,', 'let sk = sw_wsk(su,'), ('sw_chain(hf,', 'sw_chain(su,'),
    ('hs(hf, &[seed, &adrs(layer, tree, WOTS_PK', 'hs(&su.h, &[seed, &adrs(layer, tree, WOTS_PK'), ('sw_xnode(hf,', 'sw_xnode(su,'),
    ('hs(hf, &[seed, &adrs(layer, tree, TREE', 'hs(&su.h, &[seed, &adrs(layer, tree, TREE'),
    ('let sk = hs(hf, &[seed, &adrs(0, tree, FORS_PRF', 'let sk = hs(&su.f, &[seed, &adrs(0, tree, FORS_PRF'),
    ('hs(hf, &[seed, &adrs(0, tree, FORS_TREE, kp, 0, i), &sk]', 'hs(&su.f, &[seed, &adrs(0, tree, FORS_TREE, kp, 0, i), &sk]'),
    ('sw_fnode(hf,', 'sw_fnode(su,'), ('hs(hf, &[seed, &adrs(0, tree, FORS_TREE, kp, z, i)', 'hs(&su.h, &[seed, &adrs(0, tree, FORS_TREE, kp, z, i)'),
    ('fn sw_keygen(hf: &Hf,', 'fn sw_keygen(su: &Suite,'), ('fn sw_sign(hf: &Hf,', 'fn sw_sign(su: &Suite,'), ('fn sw_verify(hf: &Hf,', 'fn sw_verify(su: &Suite,'),
    ('let r = hs(hf, &[skp, seed, m], N); let dg = hs(hf, &[&r, seed, root, m], 34);', 'let r = hs(&su.h, &[skp, seed, m], N); let dg = hs(&su.h, &[&r, seed, root, m], 34);'),
    ('sig.extend(hs(hf, &[seed, &adrs(0, ix.itree, FORS_PRF', 'sig.extend(hs(&su.f, &[seed, &adrs(0, ix.itree, FORS_PRF'),
    ('hs(hf, &[seed, &adrs(0, ix.itree, FORS_ROOTS', 'hs(&su.h, &[seed, &adrs(0, ix.itree, FORS_ROOTS'),
    ('let dg = hs(hf, &[&sig[0..N], seed, root, m], 34);', 'let dg = hs(&su.h, &[&sig[0..N], seed, root, m], 34);'),
    ('let mut nd = hs(hf, &[seed, &adrs(0, ix.itree, FORS_TREE, ix.ileaf, 0, gi), &sig[o..o + N]], N);', 'let mut nd = hs(&su.f, &[seed, &adrs(0, ix.itree, FORS_TREE, ix.ileaf, 0, gi), &sig[o..o + N]], N);'),
    ('{ hs(hf, &[seed, &a, &nd, au], N) } else { hs(hf, &[seed, &a, au, &nd], N) }', '{ hs(&su.h, &[seed, &a, &nd, au], N) } else { hs(&su.h, &[seed, &a, au, &nd], N) }'),
    ('let mut nd = hs(hf, &[seed, &adrs(l, t, WOTS_PK', 'let mut nd = hs(&su.h, &[seed, &adrs(l, t, WOTS_PK')]:
    assert a in sw, a; sw = sw.replace(a, c)
b = b.replace(sw_old, sw)
b = b[:b.index('// ================= the job graph')] + open(GR, newline='').read().replace('\r\n', '\n')
# ---- main
def R(a, c, cnt=1):
    global m
    assert m.count(a) == cnt, (m.count(a), a[:70]); m = m.replace(a, c)
R('''    let fwlen = 1usize << 20; let hf = hf_of(&mode);''', '''    let fwlen = 1usize << 20; let su = suite_of(&mode); let hf = su.f;
    OLD = args.get(5).map_or(false, |s| s == "old");''')
R('''println!("RESULT sign atoms={} engine_mhz={} mode={} hash={} board={} reps={} ids={}", n, emhz, mode, hf.name, cfg!(target_arch = "aarch64"), reps,''',
  '''println!("RESULT sign3 atoms={} engine_mhz={} mode={} hash={} split_layout={} board={} reps={} ids={}", n, emhz, mode, su.name, if OLD { "old(ruty_sign.rs)" } else { "word" }, cfg!(target_arch = "aarch64"), reps,''')
R('''every hash call uses {} instead of SHAKE256. This is NOT FIPS 205 and NOT SLH-DSA; no external reference exists. Checked only for internal consistency.", hf.name); }''',
  '''every hash call uses {} instead of SHAKE256. This is NOT FIPS 205 and NOT SLH-DSA; no external reference exists. Checked only for internal consistency.", su.name); }
    if mode == "hybrid" { println!("RESULT NOTE design choices (ours, not a standard): PRF and F (64-byte inputs, every WOTS+ chain step, FORS secret values and leaves) are MiniSHAKE128 (domain byte 0x01) on split atoms; H, T_len, T_k, H_msg and PRF_msg are TurboSHAKE256 (domain byte 0x1F) on wide atoms; the width is set per batch by register writes. n = 16 output bytes everywhere. NOT FIPS 205."); }''')
R('''on split atoms (two Keccak-p[800] per atom); F and PRF (64 B) fit one 68-byte block, H (80 B), T_len (608 B), T_k (576 B), H_msg and PRF_msg take several blocks; a single call is paired with a copy of itself to fill the second core; chains run inside the atom, two at a time with equal step counts."); }''',
  '''on split atoms (two Keccak-p[800] per atom); F and PRF (64 B) fit one 68-byte block, H (80 B), T_len (608 B), T_k (576 B), H_msg and PRF_msg take several blocks; with the word layout two XMSS trees share each node so no tree call is paired with a copy of itself; calls that are alone by dependency (PRF_msg, H_msg, T_k, odd WOTS+ chain-length groups, every verify step) are paired with a copy; chains run inside the atom, two at a time with equal step counts."); }''')
R('atom_keygen(&engs, hf, sks, seed); let swroot = sw_keygen(&hf, sks, seed);', 'atom_keygen(&engs, su, sks, seed); let swroot = sw_keygen(&su, sks, seed);')
R('atom_sign(&engs, hf, sks, skp, seed, &root, &mp);\n', 'atom_sign(&engs, su, sks, skp, seed, &root, &mp);\n')
R('sw_sign(&hf, sks, skp, seed, &swroot, &mp)', 'sw_sign(&su, sks, skp, seed, &swroot, &mp)')
R('atom_verify(&engs, hf, seed, &root, &mp, &sig);', 'atom_verify(&engs, su, seed, &root, &mp, &sig, 0);', 2)
R('sw_verify(&hf, seed, &root, &mp, &sig)', 'sw_verify(&su, seed, &root, &mp, &sig)')
R('atom_verify(&engs, hf, seed, &root, &mp, &s2).0', 'atom_verify(&engs, su, seed, &root, &mp, &s2, 0).0')
R('sw_verify(&hf, seed, &root, &mp, &s2)', 'sw_verify(&su, seed, &root, &mp, &s2)')
R('atom_verify(&engs, hf, seed, &root, &m2, &sig).0; let swrejm = !sw_verify(&hf,', 'atom_verify(&engs, su, seed, &root, &m2, &sig, 0).0; let swrejm = !sw_verify(&su,')
R('atom_keygen(&engs, hf, sks, seed); tk.push', 'atom_keygen(&engs, su, sks, seed); tk.push')
R('atom_sign(&engs, hf, sks, skp, seed, &root, &mp); ts.push', 'atom_sign(&engs, su, sks, skp, seed, &root, &mp); ts.push')
R('''        bad += tbad;
        println!("RESULT time v={} mode={} op=keygen''', '''        // verify spread over fewer atoms (latency-bound: fewer, larger batches)
        let mut vp: Vec<(usize, Vec<f64>)> = Vec::new();
        for &pp in [1usize, 2].iter() { if pp >= n { continue; } let mut tt = Vec::new();
            for _ in 0..reps { let t = now_ns(); let (o2, _, ec) = atom_verify(&engs, su, seed, &root, &mp, &sig, pp); tt.push((now_ns() - t) / 1e6); if !o2 || ec != 0 { tbad += 1; } } vp.push((pp, tt)); }
        bad += tbad;
        println!("RESULT time v={} mode={} op=keygen''')
R('''        println!("RESULT time v={} mode={} op=verify atoms={} mhz={} {} wrong={}", vi, mode, n, emhz, stats(&tv), tbad);''',
  '''        println!("RESULT time v={} mode={} op=verify atoms={} mhz={} {} wrong={}", vi, mode, n, emhz, stats(&tv), tbad);
        for &(pp, ref tt) in vp.iter() { println!("RESULT time v={} mode={} op=verify_on_{}_of_{}_atoms mhz={} {} wrong={}", vi, mode, pp, n, emhz, stats(tt), tbad); }
        prof("sign", &vn, &sg_s); prof("verify", &vn, &vf_s);''')
R('''fn unhex(''', '''fn prof(name: &str, v: &str, ss: &[Stats]) { let k = ss.len() as f64; let mut tot = 0.0;
    for r in 0..NR { let f = ss.iter().map(|s| s.fill_ns[r]).sum::<f64>() / k; let c = ss.iter().map(|s| s.coll_ns[r]).sum::<f64>() / k; let n = ss.iter().map(|s| s.calls[r]).sum::<f64>() / k;
        let p = ss.iter().map(|s| s.pads[r]).sum::<f64>() / k; tot += f + c; if n == 0.0 { continue; }
        println!("RESULT profile v={} op={} routine={} calls={:.0} pad_copies={:.0} fill_ms={:.4} collect_ms={:.4} fill_ns_per_call={:.1} collect_ns_per_call={:.1}", v, name, RNAME[r], n, p, f / 1e6, c / 1e6, f / n, c / n); }
    let fl = ss.iter().map(|s| s.flush_ns).sum::<f64>() / k; let rg = ss.iter().map(|s| s.regs_ns).sum::<f64>() / k; let asm = ss.iter().map(|s| s.asm_ns).sum::<f64>() / k;
    let calls = (0..NR).map(|r| ss.iter().map(|s| s.calls[r]).sum::<f64>() / k).sum::<f64>();
    println!("RESULT profile v={} op={} fill+collect_ms={:.4} cache_flush_ms={:.4} register_writes_ms={:.4} assembly_ms={:.4} layout_total_ms={:.4} calls={:.0} layout_ns_per_call={:.1}", v, name, tot / 1e6, fl / 1e6, rg / 1e6, asm / 1e6, (tot + fl + rg + asm) / 1e6, calls, (tot + fl + rg + asm) / calls); }
fn unhex(''')
R('''    let hf = hf_of("std"); let mut rng = Rng(0xF1F1_2026_1126_0001);''', '''    let hf = hf_of("std"); let su = suite_of("std"); let mut rng = Rng(0xF1F1_2026_1126_0001);''')
m = m.replace('atom_keygen(engs, hf,', 'atom_keygen(engs, su,').replace('sw_keygen(&hf,', 'sw_keygen(&su,').replace('atom_sign(engs, hf,', 'atom_sign(engs, su,').replace('sw_sign(&hf,', 'sw_sign(&su,').replace('sw_verify(&hf,', 'sw_verify(&su,')
m = re.sub(r'atom_verify\(engs, hf, (&seed, &root, &mprime\(&[a-z0-9]+\), &(?:sig|s2))\)', r'atom_verify(engs, su, \1, 0)', m)
assert 'engs, hf' not in m and '(&hf,' not in m
hdr = '''
// ruty_sign3 (NEW, from ruty_sign.rs): adds the hybrid suite, a word-wise split-atom layout (arg 5 "old" restores the ruty_sign.rs layout),
// two XMSS trees per node in the pure split mode, a per-routine processor profile, and verify on fewer atoms.
//   sudo ./ruty_sign3 <atoms> <engine MHz> <std|fw|turbo|mini|hybrid> [repetitions] [old|word]
'''
open(DST, 'w', newline='\n').write(head + hdr + b + m)
print('ok')
