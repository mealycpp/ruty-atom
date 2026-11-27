#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# build_sign4.py: makes ruty_sign4.rs (NEW) from ruty_sign3.rs (left unchanged). Adds: firmware authentication in every mode
# (fw, fw-turbo, fw-mini, fw-hybrid), two tenants running full applications at once (mode tenants), and split-atom verify chains in
# one batch per layer (pairs of equal start digit). usage: python build_sign4.py ruty_sign3.rs ruty_sign4.rs
import sys
SRC, DST = sys.argv[1], sys.argv[2]
s = open(SRC, newline='').read().replace('\r\n', '\n')
def R(a, c, cnt=1):
    global s
    assert s.count(a) == cnt, (s.count(a), a[:80]); s = s.replace(a, c)
R('''// ruty_sign3 (NEW, from ruty_sign.rs)''', '''// ruty_sign4 (NEW, from ruty_sign3.rs): firmware authentication in every mode (fw = FIPS 205 HashSLH-DSA with SHAKE256; fw-turbo, fw-mini,
// fw-hybrid = NON-STANDARD: pre-hash TurboSHAKE256-512 of the image on a wide atom, signature in that mode, no OID exists, the OID field is
// 11 zero bytes), mode tenants (4-atom image: SLH-DSA sign and firmware verify at once on disjoint atoms, two A53 cores), and split-atom
// verify chains in one batch per layer (pairs with equal start digit; a chain without a partner is paired with a copy of itself).
//   sudo ./ruty_sign4 <atoms> <engine MHz> <std|turbo|mini|hybrid|fw|fw-turbo|fw-mini|fw-hybrid|tenants> [repetitions] [old|word]
// ruty_sign3 (NEW, from ruty_sign.rs)''')
R('''    let sw: [u32; 4] = [std::ptr::read_unaligned(seed.as_ptr() as *const u32),''', '''    if let Nk::FwH = k { let s = if FWTURBO { hf_of("turbo") } else { hf_of("std") }; return Job { g: s.g, dom: s.dom, dual: false, len: st.fwlen, olen: 64, cend: 0, cnt: 1, pc: 1, inp: st.fwpa }; }
    let sw: [u32; 4] = [std::ptr::read_unaligned(seed.as_ptr() as *const u32),''')
R('''static mut OLD: bool = false;''', '''static mut FWTURBO: bool = false;       // firmware pre-hash: false SHAKE256 (FIPS 205), true TurboSHAKE256 (NON-STANDARD modes)
static mut OLD: bool = false;''')
R('''        Nk::FwH => { let s = hf_of("std"); Job {''', '''        Nk::FwH => { let s = if FWTURBO { hf_of("turbo") } else { hf_of("std") }; Job {''')
R('''    if st.su.f.dual { (0..WL).filter(|&c| dd[c] as usize == g && dd[c] != 15).collect() }''',
  '''    if st.su.f.dual { let _ = g; let mut by: Vec<Vec<usize>> = vec![Vec::new(); 15]; for c in 0..WL { if dd[c] != 15 { by[dd[c] as usize].push(c); } }
        let mut o = Vec::new(); for v in by.iter() { for &c in v.iter() { o.push(c); } if v.len() % 2 == 1 { o.push(v[v.len() - 1]); } } o }''')
R('''    let mut pv = fr; let ng = if st.su.f.dual { 15 } else { st.np };''', '''    let mut pv = fr; let ng = if st.su.f.dual { 1 } else { st.np };''')
# ---- main: modes
R('''    let fwlen = 1usize << 20; let su = suite_of(&mode); let hf = su.f;''', '''    let fwlen = 1usize << 20; let su = suite_of(mode.trim_start_matches("fw-")); let hf = su.f;''')
R('''    let standard = mode == "std" || mode == "fw";''', '''    let standard = mode == "std" || mode == "fw" || mode == "tenants";''')
R('''    if mode == "fw" { fw_main(&engs, fva, fpa, fwlen, reps, &mut bad); println!("RESULT summary mode=fw mismatches={} {}", bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" }); return; }''',
  '''    if mode.starts_with("fw") { fw_main(&engs, fva, fpa, fwlen, reps, &mut bad, &mode); println!("RESULT summary mode={} mismatches={} {}", mode, bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" }); return; }
    if mode == "tenants" { tenants_main(&engs, fva, fpa, fwlen, reps, &mut bad); println!("RESULT summary mode=tenants mismatches={} {}", bad, if bad == 0 { "ALL_MATCH" } else { "FAIL" }); return; }''')
# ---- fw_main in every mode
R('''unsafe fn fw_main(engs: &[Eng], fva: *mut u8, fpa: u64, fwlen: usize, reps: usize, bad: &mut usize) {
    let hf = hf_of("std"); let su = suite_of("std"); let mut rng = Rng(0xF1F1_2026_1126_0001);''',
  '''unsafe fn fw_main(engs: &[Eng], fva: *mut u8, fpa: u64, fwlen: usize, reps: usize, bad: &mut usize, mode: &str) {
    FWTURBO = mode != "fw"; let hf = if FWTURBO { hf_of("turbo") } else { hf_of("std") }; let su = suite_of(mode.trim_start_matches("fw-")); let mut rng = Rng(0xF1F1_2026_1126_0001);
    let oid: [u8; 11] = if FWTURBO { [0u8; 11] } else { OID_SHAKE256 };''')
R('''    let mprime = |dig: &[u8]| { let mut m = vec![1u8, 0u8]; m.extend_from_slice(&OID_SHAKE256); m.extend_from_slice(dig); m };''',
  '''    let mprime = |dig: &[u8]| { let mut m = vec![1u8, 0u8]; m.extend_from_slice(&oid); m.extend_from_slice(dig); m };''')
R('''    let swdig = xofd(&hf.g, 0x1F, &image, 64);
    println!("KEY v=fw mode=fw sk_seed={} sk_prf={} pk_seed={} pk_root={} image_rng_seed=0xF1F120261126_0001 image_len={} image_shake256_512={}", hx(&sks), hx(&skp), hx(&seed), hx(&root), fwlen, hx(&swdig));''',
  '''    let swdig = xofd(&hf.g, hf.dom, &image, 64);
    println!("KEY v=fw mode={} sk_seed={} sk_prf={} pk_seed={} pk_root={} image_rng_seed=0xF1F120261126_0001 image_len={} image_{}_512={}", mode, hx(&sks), hx(&skp), hx(&seed), hx(&root), fwlen, hf.name, hx(&swdig));''')
R('''    println!("SIG v=fw mode=fw len={} hex={}", sig.len(), hx(&sig));''', '''    println!("SIG v=fw mode={} len={} hex={}", mode, sig.len(), hx(&sig));''')
R('''    println!("RESULT check v=fw keygen_match_sw''', '''    println!("RESULT check v=fw mode={} prehash={} keygen_match_sw''')
R('''        root == swroot, dig == swdig, sig == swsig, okv, rej_img, rej_sig, swok, e, if ok { "PASS" } else { "FAIL" });''',
  '''        mode, hf.name, root == swroot, dig == swdig, sig == swsig, okv, rej_img, rej_sig, swok, e, if ok { "PASS" } else { "FAIL" });''')
for op in ['signer_total', 'signer_hash_1MB', 'signer_sign', 'verifier_total', 'verifier_hash_1MB', 'verifier_verify']:
    R('"RESULT time v=fw op=%s {}' % op, '"RESULT time v=fw mode={} op=%s {}' % op)
R('''stats(&st_), tbad); println!("RESULT time v=fw mode={} op=signer_hash_1MB {}", stats(&sh)); println!("RESULT time v=fw mode={} op=signer_sign {}", stats(&ss));''',
  '''mode, stats(&st_), tbad); println!("RESULT time v=fw mode={} op=signer_hash_1MB {}", mode, stats(&sh)); println!("RESULT time v=fw mode={} op=signer_sign {}", mode, stats(&ss));''')
R('''stats(&vt), tbad); println!("RESULT time v=fw mode={} op=verifier_hash_1MB {}", stats(&vh)); println!("RESULT time v=fw mode={} op=verifier_verify {}", stats(&vv)); }''',
  '''mode, stats(&vt), tbad); println!("RESULT time v=fw mode={} op=verifier_hash_1MB {}", mode, stats(&vh)); println!("RESULT time v=fw mode={} op=verifier_verify {}", mode, stats(&vv)); }''')
s += r'''
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
'''
open(DST, 'w', newline='\n').write(s); print('ok')
