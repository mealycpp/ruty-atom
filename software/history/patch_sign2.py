# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# makes ruty_sign2.rs from ruty_sign.rs: constant-size 4-byte copies in the split-atom byte layout (put and getd); nothing else changes
import sys
src, dst = sys.argv[1], sys.argv[2]
s = open(src, newline='').read().replace('\r\n', '\n')
a = "        while c < n { let p = off + c; let q = p % 4; let ch = if 4 - q < n - c { 4 - q } else { n - c };\n            std::ptr::copy_nonoverlapping(src.as_ptr().add(c), b.add((p / 4) * 8 + q), ch); c += ch; } } }"
b = "        while c < n { let p = off + c; let q = p % 4; if q == 0 && n - c >= 4 { std::ptr::copy_nonoverlapping(src.as_ptr().add(c), b.add((p / 4) * 8), 4); c += 4; continue; }\n            let ch = if 4 - q < n - c { 4 - q } else { n - c }; std::ptr::copy_nonoverlapping(src.as_ptr().add(c), b.add((p / 4) * 8 + q), ch); c += ch; } } }"
c = "let mut c = 0; while c < n { let ch = if n - c < 4 { n - c } else { 4 }; std::ptr::copy_nonoverlapping(b.add((c / 4) * 8), dst.as_mut_ptr().add(c), ch); c += 4; } }"
d = "let mut c = 0; while c < n { if n - c >= 4 { std::ptr::copy_nonoverlapping(b.add((c / 4) * 8), dst.as_mut_ptr().add(c), 4); } else { std::ptr::copy_nonoverlapping(b.add((c / 4) * 8), dst.as_mut_ptr().add(c), n - c); } c += 4; } }"
e = "// ruty_sign: a COMPLETE"
f = "// ruty_sign2 = ruty_sign.rs with one change only: constant-size 4-byte copies in the split-atom byte layout (Wr::put, getd).\n// ruty_sign: a COMPLETE"
for x, y in ((a, b), (c, d), (e, f)):
    assert s.count(x) == 1, x[:40]; s = s.replace(x, y)
open(dst, 'w', newline='\n').write(s); print('ok')
