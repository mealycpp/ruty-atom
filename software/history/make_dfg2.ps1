# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Build board\ruty_dfg2.rs: the job-graph scheduler with a no-copy mode for stream nodes (the atom reads the tenant's buffer in place).
$b = "C:\Users\elhad\ruty\board"
$t = Get-Content "$b\ruty_dfg_tail.rs" -Raw
function Rep([string]$s, [string]$old, [string]$new) { $c = ([regex]::Matches($s, [regex]::Escape($old))).Count; if ($c -ne 1) { Write-Output "REPLACE COUNT $c for: $($old.Substring(0, [Math]::Min(60, $old.Length)))" }; return $s.Replace($old, $new) }
$t = Rep $t "const TAGB: usize = 2400;" "static mut ZC: bool = false; static mut TAG_VA: *mut u8 = std::ptr::null_mut(); static mut FW_VA: *mut u8 = std::ptr::null_mut(); static mut TAG_PA: u64 = 0; static mut FW_PA: u64 = 0;`nconst TAGSTR: usize = 241664;          // tag nodes sit 59 pages apart in the tenant buffer`nunsafe fn win(e: &Eng, pa: u64, len: usize) { let lo = if pa < e.pa { pa } else { e.pa }; let h1 = pa + len as u64; let h2 = e.pa + PAGE as u64; let hi = if h1 > h2 { h1 } else { h2 };`n    e.wr(WBASE, (lo >> 12) as u32); e.wr(WLIMIT, ((hi + 4095) >> 12) as u32); }`nconst TAGB: usize = 2400;"
$t = Rep $t "if let Some(ni) = pick { let a = nodes[ni].a;" "if let Some(ni) = pick { let a = nodes[ni].a; let mut inp = e.buf(0).1;"
$t = Rep $t "Kind::Tags => { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(tag_in.as_ptr().add(a * TAGIN), p, TAGIN); to_device(p, TAGIN); let m = &ALGS[4];" "Kind::Tags => { if ZC { let sp = TAG_PA + (a * TAGSTR) as u64; to_device(TAG_VA.add(a * TAGSTR), TAGIN); win(e, sp, TAGIN); inp = sp; } else { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(tag_in.as_ptr().add(a * TAGIN), p, TAGIN); to_device(p, TAGIN); } let m = &ALGS[4];"
$t = Rep $t "Kind::Stream => { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(fw_in.as_ptr().add(a * FWLEN), p, FWLEN); to_device(p, FWLEN); let m = &ALGS[2];" "Kind::Stream => { if ZC { let sp = FW_PA + (a * FWLEN) as u64; to_device(FW_VA.add(a * FWLEN), FWLEN); win(e, sp, FWLEN); inp = sp; } else { let p = e.buf(0).0; std::ptr::copy_nonoverlapping(fw_in.as_ptr().add(a * FWLEN), p, FWLEN); to_device(p, FWLEN); } let m = &ALGS[2];"
$t = Rep $t "e.start(e.buf(0).1, e.buf(1).1, nj); since[ai] = now_ns();" "e.start(inp, e.buf(1).1, nj); since[ai] = now_ns();"
$alloc = @'
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
'@
$t = Rep $t '    let names = ["serial", "graph", "graph+rule"]; let mut allbad = 0usize;' $alloc
$t = Rep $t '    println!("RESULT dfg summary' "    }`n    println!(`"RESULT dfg summary"
$h = Get-Content "$b\ruty_apps.rs" -TotalCount 87; $m = Get-Content "$b\ruty_sched_fast.rs" -TotalCount 47
(($h + $m) -join "`n") + "`n" + $t | Set-Content -Encoding ascii "$b\ruty_dfg2.rs" -NoNewline
(Get-Content "$b\run_dfg.sh" -Raw).Replace("ruty_dfg", "ruty_dfg2").Replace("dfg_`$1.log", "dfg2_`$1.log").Replace("run_dfg2.sh", "run_dfg2.sh") | Set-Content -Encoding ascii "$b\run_dfg2.sh" -NoNewline
"lines: " + (Get-Content "$b\ruty_dfg2.rs" | Measure-Object -Line).Lines
Select-String -Path "$b\run_dfg2.sh" -Pattern "rustc|tee" | ForEach-Object { $_.Line }
