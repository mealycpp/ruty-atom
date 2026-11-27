# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty.rs: buffers come from the kernel's contiguous-memory heap (/dev/dma_heap/reserved), which lies in the low 2 GB that the
# memory port of the image reaches. Huge pages are dropped: on this 8 GB board they land above 2 GB after a fresh boot.
import re
s = open("ruty.rs").read()
new = """let mut a = HeapAlloc { len: PAGE as u64, fd: 0, fd_flags: 2 | 0x80000, heap_flags: 0 };            // one contiguous 2 MB buffer
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
        let g = Eng { reg, va, pa };"""
m = re.search(r"let \(mut va, mut pa\) = .*?let g = Eng \{ reg, va, pa \};", s, re.S); assert m
s = s[:m.start()] + new + s[m.end():]
a = "    for i in 0..n {\n        let reg = mmap("; assert s.count(a) == 1
s = s.replace(a, "    let hf = open(b\"/dev/dma_heap/reserved\\0\".as_ptr() as *const c_char, 2); if hf < 0 { println!(\"RESULT error=open_dma_heap\"); return; }\n" + a)
a = "\nconst LIMIT: u64"; assert s.count(a) == 1
s = s.replace(a, "\n#[repr(C)] struct HeapAlloc { len: u64, fd: u32, fd_flags: u32, heap_flags: u64 }\nextern \"C\" { fn ioctl(fd: c_int, req: u64, arg: *mut HeapAlloc) -> c_int; }" + a)
open("ruty.rs", "w").write(s)
r = open("run_rust.sh").read().replace("echo 400 > /proc/sys/vm/nr_hugepages; grep HugePages_Total /proc/meminfo", "grep CmaFree /proc/meminfo")
open("run_rust.sh", "w").write(r)
