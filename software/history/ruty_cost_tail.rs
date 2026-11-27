// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy

// ================= ruty_cost: the processor-side routines of the job-graph scheduler, timed one by one =================
// The same binary runs on the board (real Cortex-A53) and in gem5 (model of it). No device is touched: registers are plain memory,
// buffers are ordinary pages. What is timed is what the processor does for a node: lay out its data, flush, collect its digests.
//   ./ruty_cost <flush 1|0> <repetitions>
fn tm<F: FnMut(usize)>(name: &str, reps: usize, mut f: F) { f(0); let mut mn = f64::MAX; let mut sum = 0.0;
    for r in 0..reps { let t0 = now_ns(); f(r + 1); let d = now_ns() - t0; if d < mn { mn = d; } sum += d; }
    println!("COST {} min_ns={:.0} mean_ns={:.0} reps={}", name, mn, sum / reps as f64, reps); }
fn main() { unsafe {
    let args: Vec<String> = std::env::args().collect(); FLUSH = args.get(1).map_or(true, |s| s != "0");
    let reps: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
    let mut regs = vec![0u32; 1024]; let lay = std::alloc::Layout::from_size_align(PAGE, 4096).unwrap(); let va = std::alloc::alloc_zeroed(lay);
    let e = Eng { reg: regs.as_mut_ptr(), va, pa: 0 };
    let seed: Vec<u8> = (0..16u8).collect(); let sks: Vec<u8> = (100..116u8).collect(); let mut trees = new_trees();
    let mut rng = Rng(0x5EED_0DF6_2026_1126); let mut tags = rng.msg(16 * 240000); let mut fw = rng.msg(4 << 20); let mut sink = vec![0u8; 38400];
    println!("COST start flush={} reps={}", FLUSH, reps);
    tm("fill_secrets_280", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 0; fill(&e, t, false, &seed, &sks); });
    tm("fill_chains_280", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 1; fill(&e, t, false, &seed, &sks); });
    tm("fill_compress_8", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 2; fill(&e, t, false, &seed, &sks); });
    tm("fill_level_4", reps, |r| { let t = &mut trees[r % NTREE]; t.stage = 3; t.nl = 8; fill(&e, t, false, &seed, &sks); });
    tm("collect_280", reps, |r| { let t = &mut trees[r % NTREE]; let ob = e.buf(1).0; from_device(ob, 280 * 16); let dst = t.ends.as_mut_ptr(); for k in 0..280 { rdg(ob, false, k, dst.add(k * 16)); } });
    tm("collect_8", reps, |r| { let t = &mut trees[r % NTREE]; let ob = e.buf(1).0; from_device(ob, 8 * 16); let dst = t.lvl.as_mut_ptr(); for k in 0..8 { rdg(ob, false, k, dst.add(k * 16)); } });
    tm("tags_copy_240kB", reps, |r| { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(tags.as_ptr().add((r % 16) * 240000), p, 240000); to_device(p, 240000); });
    tm("tags_collect_38kB", reps, |_r| { let ob = e.buf(1).0; from_device(ob, 38400); std::ptr::copy_nonoverlapping(ob, sink.as_mut_ptr(), 38400); });
    tm("firmware_copy_1MB", reps, |r| { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(fw.as_ptr().add((r % 4) << 20), p, 1 << 20); to_device(p, 1 << 20); });
    tm("tags_flush_only_240kB", reps, |r| { let p = tags.as_mut_ptr().add((r % 16) * 240000); *p = (*p).wrapping_add(1); to_device(p, 240000); });
    tm("firmware_flush_only_1MB", reps, |r| { let p = fw.as_mut_ptr().add((r % 4) << 20); *p = (*p).wrapping_add(1); to_device(p, 1 << 20); });
    let mut acc = 0u32;
    tm("poll_1000_reads", reps, |_r| { for _ in 0..1000 { acc = acc.wrapping_add(e.rd(STATUS)); } });
    tm("shape_5_writes", reps, |r| { e.wr(LEN, r as u32); e.wr(OLEN, 16); e.wr(ALG, 1); e.wr(0x44, 0); e.wr(0x48, 15); });
    println!("COST end acc={} sink={}", acc, sink[0]);
} }
