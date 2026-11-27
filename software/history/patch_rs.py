# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# ruty.rs: more message lengths in the shared-port test, and the status of an engine that does not finish is printed.
s = open("ruty.rs").read()
a = "[(0usize, 32usize, 20000usize), (2, 32, 20000), (4, 32, 20000), (0, 4096, 200)]"
b = "[(0usize, 32usize, 20000usize), (2, 32, 20000), (4, 32, 20000), (0, 136, 6000), (0, 1024, 800), (0, 4096, 200), (2, 4096, 200)]"
assert s.count(a) == 1; s = s.replace(a, b)
a = "let mut ok = true; for x in engs[..k].iter() { ok &= x.wait() & 0x72 == 2; }"
b = "let mut ok = true; for (i, x) in engs[..k].iter().enumerate() { let st = x.wait(); if st & 0x72 != 2 { println!(\"RESULT port_error engine={} status={:08x}\", i, st); } ok &= st & 0x72 == 2; }"
assert s.count(a) == 1; s = s.replace(a, b)
open("ruty.rs", "w").write(s)
