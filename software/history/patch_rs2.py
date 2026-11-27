# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty.rs: the memory port of the image reaches the low 2 GB only. Take huge pages until one lies there (the board has 8 GB).
import re
s = open("ruty.rs").read()
new = """let (mut va, mut pa) = (std::ptr::null_mut::<u8>(), 0u64);                                // a huge page the memory port can reach
        for _try in 0..400 {
            va = mmap(std::ptr::null_mut(), PAGE, 3, 0x02 | 0x20 | 0x40000, -1, 0) as *mut u8;           // private, anonymous, huge page
            if va as isize == -1 { println!("RESULT error=no_reachable_huge_page (raise /proc/sys/vm/nr_hugepages)"); return; }
            *va = 0; mlock(va as *const c_void, PAGE);
            let mut e = [0u8; 8]; pm.seek(SeekFrom::Start((va as u64 / 4096) * 8)).unwrap(); pm.read_exact(&mut e).unwrap();
            let ent = u64::from_le_bytes(e); if ent >> 63 == 0 || ent & ((1 << 55) - 1) == 0 { println!("RESULT error=no_physical_address (run as root)"); return; }
            pa = (ent & ((1 << 55) - 1)) * 4096; if pa + (PAGE as u64) <= LIMIT { break; }
        }
        if pa + (PAGE as u64) > LIMIT { println!("RESULT error=no_reachable_huge_page"); return; }
        let g = Eng { reg, va, pa };"""
m = re.search(r"let va = mmap\(.*?let g = Eng \{ reg, va, pa \};", s, re.S); assert m
s = s[:m.start()] + new + s[m.end():]
assert s.count("\nfn main() {") == 1; s = s.replace("\nfn main() {", "\nconst LIMIT: u64 = 0x8000_0000;                          // the image maps the low 2 GB of memory to the engines\nfn main() {")
open("ruty.rs", "w").write(s)
r = open("run_rust.sh").read(); print([l for l in r.split("\n") if "hugepages" in l])
