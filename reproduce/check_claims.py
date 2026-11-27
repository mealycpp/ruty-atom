#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""check_claims.py: recompute the numerical claims of the RUTY-ATOM letter from the raw evidence in this repository.

    python3 reproduce/check_claims.py            # every check, one line each
    python3 reproduce/check_claims.py --quiet    # only FAIL and MISSING lines, then the summary

Rules this script follows
  * A measured value is never typed in here. It is read from a board log (results/), a Vivado report (synthesis/reports/)
    or the simulator (model/). The only constants in this file are the values PRINTED IN THE LETTER, which are what is
    being tested, a few published figures of other authors, and physical constants of the build (300 MHz bus clock).
  * Table I is checked twice. First against the session the letter quotes (results/sign4_*, modes_*), whose logs hold the
    median of 25 runs. Then against a second session (results/sign5_*, modes2_*) that logs all 25 times: the median is
    recomputed from them and must agree with the letter within the 8% the letter states for session-to-session movement.
  * A check PASSES when the recomputed value, rounded to the number of digits the letter prints, equals the letter's
    value. The unrounded value, the file it came from and the calculation are always printed.
  * FAIL    : the evidence is there and gives a different value.
  * MISSING : the evidence the claim needs is not in the repository.
  * Exit code 0 only if every mandatory check passes. Checks marked "(info)" do not affect the exit code.
Only the Python standard library is used.
"""
import math, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RES = os.path.join(ROOT, "results")
REP = os.path.join(ROOT, "synthesis", "reports")
SIM = os.path.join(ROOT, "model", "sim")
G5 = os.path.join(ROOT, "model", "gem5_atom")
QUIET = "--quiet" in sys.argv
BUS_MHZ = 300.0          # the CLOCKS register counts bus clocks (rtl/atom/ruty_top3.v, register 0x34); checked against clocks.rpt below
ATOM_ID = "52552021"     # identification word of the atom build the letter reports (printed by every board program)
N_CLOCK = {1: 450, 2: 420, 4: 400}

rows = []                # (section, status, mandatory, claim, letter, got, source, calc)
_section = [""]


class Missing(Exception):
    pass


def section(name):
    _section[0] = name


def add(status, claim, letter, got, source, calc="", mandatory=True):
    rows.append((_section[0], status, mandatory, claim, str(letter), str(got), source, calc))


def read(path):
    """Text of a file under the repository; UTF-16 files written by PowerShell are decoded as such."""
    full = path if os.path.isabs(path) else os.path.join(ROOT, path)
    if not os.path.isfile(full):
        raise Missing(os.path.relpath(full, ROOT))
    raw = open(full, "rb").read()
    if raw[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return raw.decode("utf-16")
    return raw.decode("utf-8", errors="replace")


def rel(path):
    return os.path.relpath(path, ROOT).replace(os.sep, "/")


def kv(line):
    """key=value fields of one log line."""
    return dict(re.findall(r"([A-Za-z_][A-Za-z0-9_+.\-]*)=([^\s|]+)", line))


def lines_of(path, prefix):
    return [l for l in read(path).splitlines() if l.startswith(prefix)]


def digits(s):
    """Number of decimals in a value as the letter prints it."""
    s = str(s)
    return len(s.split(".")[1]) if "." in s else 0


def rnd(x, nd):
    """Round half away from zero, as a reader would, on the shortest decimal form of x (avoids 2.675 -> 2.67)."""
    q = 10 ** nd
    return math.floor(abs(float(repr(x))) * q + 0.5 + 1e-9) / q * (1 if x >= 0 else -1)


def check(claim, letter, value, source, calc="", mandatory=True, unit=""):
    """Compare a recomputed value with the letter's printed value at the letter's precision."""
    nd = digits(letter)
    ok = abs(rnd(value, nd) - float(letter)) < 10 ** (-nd) / 2
    add("PASS" if ok else "FAIL", claim, "%s%s" % (letter, unit), ("%.*f%s" % (nd + 3, value, unit)), source, calc, mandatory)
    return ok


def check_true(claim, cond, letter, got, source, calc="", mandatory=True):
    add("PASS" if cond else "FAIL", claim, letter, got, source, calc, mandatory)
    return cond


def guard(fn):
    """Run one group of checks; a missing file becomes a MISSING line instead of a crash."""
    try:
        fn()
    except Missing as e:
        add("MISSING", fn.__doc__.strip().split("\n")[0], "-", "-", str(e), "file not in the repository")
    except Exception as e:                                    # a log that does not have the expected line
        add("MISSING", fn.__doc__.strip().split("\n")[0], "-", "-", "-", "could not parse the evidence: %s: %s" % (type(e).__name__, e))


# ------------------------------------------------------------------------------------------------ evidence readers
def util(build):
    """(system LUTs, atom-block LUTs, atom-block flip-flops, tool version) of one build, from its routed utilization report."""
    p = os.path.join(REP, build, "route_util_hier.rpt")
    t = read(p)
    top = atom = None
    for l in t.splitlines():
        c = [x.strip() for x in l.split("|")]
        if len(c) > 8 and c[2] == "(top)":
            top = int(c[3])
        if len(c) > 8 and c[1] == "inst" and c[2].endswith("ruty_top") and atom is None:
            atom = (int(c[3]), int(c[7]))
    ver = re.search(r"Tool Version\s*:\s*(Vivado v\.[0-9.]+)", t)
    if top is None or atom is None:
        raise ValueError("no utilization rows in " + rel(p))
    return top, atom[0], atom[1], ver.group(1) if ver else "?", rel(p)


def timing(build):
    """(atom clock MHz, bus clock MHz, worst setup slack, worst hold slack, 'constraints met' line present) of one routed build."""
    p = os.path.join(REP, build, "route_timing.rpt")
    t = read(p)
    m = re.search(r"WNS\(ns\)\s+TNS\(ns\).*?\n\s*-+.*?\n\s*(-?[0-9.]+)\s+(-?[0-9.]+)\s+\d+\s+\d+\s+(-?[0-9.]+)", t, re.S)
    met = "All user specified timing constraints are met." in t
    cl = dict(re.findall(r"^\s*(clk_pl_0|clk_out1_ruty_clk_wiz_0_0)\s+\{[^}]*\}\s+([0-9.]+)\s+[0-9.]+", t, re.M))
    return 1000.0 / float(cl["clk_out1_ruty_clk_wiz_0_0"]), 1000.0 / float(cl["clk_pl_0"]), float(m.group(1)), float(m.group(3)), met, rel(p)


def time_line(path, v, op, mode=None):
    """One 'RESULT time' line: the board program's statistics over n runs of one operation on one test case."""
    for l in lines_of(path, "RESULT time"):
        d = kv(l)
        if d.get("v") == str(v) and d.get("op") == op and (mode is None or d.get("mode") == mode):
            return d
    raise ValueError("no 'RESULT time v=%s op=%s' in %s" % (v, op, path))


def med(path, v, op):
    return float(time_line(path, v, op)["median_ms"])


def header_ids(path):
    ids = set()
    for l in read(path).splitlines():
        m = re.search(r"\bids?=([0-9a-f ]+?)(?:\s+[A-Za-z_]+=|\s*$)", l)
        if m:
            ids.update(m.group(1).split())
    return ids


# ------------------------------------------------------------------------------------------------ 1. cost of the second width
def cost():
    """Cost of the second width (LUTs, flip-flops, clock)"""
    _, s_lut, s_ff, ver, sp = util("atom_v3_n1_m16_300_450")
    _, w_lut, w_ff, _, wp = util("atom_fix2w_n1_m16_300_450")
    check_true("splittable atom block, LUTs", s_lut == 8064, "8,064", s_lut, sp, "row 'inst ... ruty_top', column Total LUTs")
    check_true("splittable atom block, flip-flops", s_ff == 4400, "4,400", s_ff, sp, "same row, column FFs")
    check_true("fixed 1x1600 atom block, LUTs", w_lut == 7419, "7,419", w_lut, wp, "row 'inst ... ruty_top', column Total LUTs")
    check_true("fixed 1x1600 atom block, flip-flops", w_ff == 4397, "4,397", w_ff, wp, "same row, column FFs")
    check_true("LUTs added by the second width", s_lut - w_lut == 645, "645", s_lut - w_lut, sp + " - " + wp, "%d - %d" % (s_lut, w_lut))
    check("LUT overhead of the second width", "8.7", 100.0 * (s_lut - w_lut) / w_lut, sp + " / " + wp, "100 * %d / %d" % (s_lut - w_lut, w_lut), unit="%")
    _, n_lut, n_ff, _, npth = util("atom_fix2n_n1_m16_300_450")
    check_true("fixed 2x800 atom block, LUTs (README)", n_lut == 7421, "7,421", n_lut, npth, "row 'inst ... ruty_top'", mandatory=False)
    top, _, _, _, _ = util("atom_v3_n1_m16_300_450")
    check_true("one-atom system, LUTs", top == 10006, "10,006", top, sp, "row 'ruty_wrapper (top)'")
    check_true("tool that produced the reports", ver.startswith("Vivado v.2022.2"), "Vivado 2022.2", ver, sp, "report header", mandatory=False)
    # the two builds at 480 MHz: both must meet timing
    for b, name in (("atom_v3_n1_m16_300_480", "splittable"), ("atom_fix2w_n1_m16_300_480", "fixed 1x1600")):
        f, _, wns, whs, met, p = timing(b)
        check_true("%s build meets timing at 480 MHz" % name, met and wns >= 0 and whs >= 0 and rnd(f, 0) >= 480, "480 MHz, met",
                   "%.1f MHz, WNS %+.3f ns, WHS %+.3f ns, %s" % (f, wns, whs, "met" if met else "NOT met"), p, "Design Timing Summary; 1000 / period of the atom clock")
    # the same comparison in the 480 MHz builds, for the reader's information
    _, s480, _, _, p1 = util("atom_v3_n1_m16_300_480")
    _, w480, _, _, p2 = util("atom_fix2w_n1_m16_300_480")
    add("PASS", "LUT overhead in the two 480 MHz builds (info: the letter's 645 is from the 450 MHz builds)", "not printed", "%d LUTs, %.1f%%" % (s480 - w480, 100.0 * (s480 - w480) / w480),
        p1 + " - " + p2, "%d - %d" % (s480, w480), mandatory=False)
    for n, b in ((1, "atom_v3_n1_m16_300_450"), (2, "atom_v3_n2_m16_300_420"), (4, "atom_v3_n4_m16_300_400")):
        f, fb, wns, whs, met, p = timing(b)
        check_true("N=%d system build: %d MHz, timing met" % (n, N_CLOCK[n]), met and wns >= 0 and whs >= 0 and rnd(f, 0) == N_CLOCK[n], "%d MHz, met" % N_CLOCK[n],
                   "%.1f MHz, WNS %+.3f ns, WHS %+.3f ns, %s" % (f, wns, whs, "met" if met else "NOT met"), p, "Design Timing Summary")
        if n == 1:
            check_true("bus and memory-port clock", rnd(fb, 0) == BUS_MHZ, "300 MHz", "%.2f MHz" % fb, p, "1000 / period of clk_pl_0")


# ------------------------------------------------------------------------------------------------ 2. correctness evidence
def vectors():
    """3,411 NIST and RFC test vectors on the hardware"""
    need = [("atom_v3_n1_450", 0), ("atom_v3_n2_420", 0), ("atom_v3_n2_420", 1), ("atom_v3_n4_400", 0), ("atom_v3_n4_400", 1), ("atom_v3_n4_400", 2), ("atom_v3_n4_400", 3)]
    got = {}; other = []
    for fn in sorted(os.listdir(RES)):
        if "kat" not in fn:
            continue
        t = read(os.path.join(RES, fn))
        head = re.search(r"RESULT id=([0-9a-f]+)[^\n]*", t)
        d = kv(head.group(0)) if head else {}
        tot = mis = mod = 0
        for l in t.splitlines():
            m = re.match(r"RESULT kat (\S+) vectors=(\d+) hardware_mismatches=(\d+) model_mismatches=(\d+)", l)
            if m:
                tot += int(m.group(2)); mis += int(m.group(3)); mod += int(m.group(4))
        if not tot:
            continue
        if d.get("id") == ATOM_ID and "image" in d and "atom" in d:
            got[(d["image"], int(d["atom"]))] = (fn, tot, mis, mod)
        else:
            other.append("%s: build id %s, %d vectors, %d hardware mismatches" % (fn, d.get("id", "?"), tot, mis))
    for image, atom in need:
        claim = "3,411 NIST and RFC vectors on %s, atom %d (build id %s)" % (image, atom, ATOM_ID)
        if (image, atom) in got:
            fn, tot, mis, mod = got[(image, atom)]
            check_true(claim, tot == 3411 and mis == 0 and mod == 0, "3,411, 0 mismatches", "%d, %d hardware mismatches (software model: %d)" % (tot, mis, mod), "results/" + fn,
                       "sum of the per-suite 'vectors=' fields; sums of 'hardware_mismatches=' and 'model_mismatches='; expected values are those of the published NIST and RFC 9861 files")
        else:
            add("MISSING", claim, "3,411, 0 mismatches", "no vector log for this atom", "results/kat3_%s_atom%d_aup1.txt" % (image, atom), "vector logs of other builds: " + ("; ".join(other) or "none"))
    add("PASS", "vector runs of earlier builds kept in the repository (info)", "not printed", "; ".join(other) or "none", "results/*kat*", "listed for completeness; the letter's claim rests on the seven logs above", mandatory=False)


def fips205():
    """Standard signatures byte-identical to an independent FIPS 205 implementation"""
    for n in (1, 2, 4):
        p = "results/check_sign4_128f_std_%datoms_aup1_fips205.txt" % n
        t = read(p)
        vs = [kv(l) for l in t.splitlines() if l.strip().startswith("v=")]
        ok = (len(vs) == 4 and all(d.get("pk_root_identical") == "True" and d.get("sig_identical") == "True" and d.get("sig_len") == "17088"
                                    and d.get("slhdsa_verify_board_sig") == "True" for d in vs))
        nist = sum(1 for d in vs if d.get("equals_NIST_ACVP_signature") == "True")
        summ = re.search(r"SUMMARY \S+ checked=(\d+) failed=(\d+)", t)
        check_true("N=%d: 4 keys and messages, public key and 17,088-byte signature identical, one NIST signature" % n,
                   ok and nist == 1 and summ and summ.group(2) == "0", "4 identical, 1 NIST, 0 failed",
                   "%d identical, %d NIST, %s failed" % (sum(1 for d in vs if d.get("sig_identical") == "True"), nist, summ.group(2) if summ else "?"), p,
                   "output of the independent check (software/check_sign_fips205.py) on the signatures in results/sign4_128f_std_%datoms_aup1.txt" % n)
        log = "results/sign4_128f_std_%datoms_aup1.txt" % n
        ch = [kv(l) for l in lines_of(log, "RESULT check")]
        check_true("N=%d: board program's own checks (software signature equal, verify accepts, 6 flipped signatures and a wrong message rejected)" % n,
                   len(ch) == 4 and all(d.get("errors") == "0" and d.get("sign_match_sw") == "true" and d.get("atom_verify_good") == "true" and d.get("atom_reject_flipped") == "6/6"
                                        and d.get("atom_reject_wrong_msg") == "true" for d in ch), "4 of 4, errors 0", "%d of 4 with errors=0" % sum(1 for d in ch if d.get("errors") == "0"), log,
                   "'RESULT check' lines")
        check_true("N=%d: every log is of the atom build of the letter" % n, header_ids(log) == {ATOM_ID}, ATOM_ID, " ".join(sorted(header_ids(log))), log, "'ids=' of the header line", mandatory=False)
    p = "results/sign4_128f_fw_1atoms_aup1.txt"
    d = kv(lines_of(p, "RESULT check")[0])
    check_true("firmware authentication: tampered image and flipped signature rejected", d.get("atom_reject_tampered_image") == "true" and d.get("atom_reject_flipped_sig") == "true" and d.get("errors") == "0",
               "rejected", "tampered=%s flipped=%s errors=%s" % (d.get("atom_reject_tampered_image"), d.get("atom_reject_flipped_sig"), d.get("errors")), p, "'RESULT check' line")
    d = kv([l for l in read(p).splitlines() if l.startswith("KEY ")][0])
    check_true("firmware image size (1 MiB)", d.get("image_len") == "1048576", "1,048,576 bytes", d.get("image_len"), p, "'KEY' line, image_len")


def width_changes():
    """Width changes between jobs and fixed-width builds on the board"""
    p = "results/atom_tenants_n4_400_aup1.txt"
    d = kv([l for l in lines_of(p, "RESULT tenants reform")][0])
    n, a, b, m = int(d["width_changes"]), int(d["wide_to_split"]), int(d["split_to_wide"]), int(d["mismatches"])
    check_true("width changes of atoms between jobs", n == 116, "116", n, p, "'RESULT tenants reform' line, width_changes")
    check_true("of which to the split state / back", (a, b) == (57, 59) and a + b == n, "57 / 59", "%d / %d (sum %d)" % (a, b, a + b), p, "wide_to_split / split_to_wide; their sum must equal width_changes")
    check_true("wrong digests across the width changes", m == 0, "0", m, p, "mismatches")
    check_true("setting width, round range and window: five register writes", d.get("shape_ns_per_atom") == "151", "151 ns", d.get("shape_ns_per_atom") + " ns", p, "shape_ns_per_atom")
    check_true("that run is of the atom build of the letter", header_ids(p) == {ATOM_ID}, ATOM_ID, " ".join(sorted(header_ids(p))), p, "'ids=' of the header line", mandatory=False)
    for fn, name in (("fixcheck_rt_n1_450_aup1.txt", "splittable"), ("fixcheck_fix2w_n1_450_aup1.txt", "fixed 1x1600"), ("fixcheck_fix2n_n1_450_aup1.txt", "fixed 2x800")):
        p = "results/" + fn
        ls = lines_of(p, "RESULT fixcheck")
        mism = sum(int(kv(l).get("mismatches", 0)) for l in ls if "mismatches=" in l)
        summ = [l for l in ls if " summary " in l]
        check_true("%s build on the board: digests and chains against software" % name, mism == 0 and summ and "ALL_MATCH" in summ[0], "0 mismatches", "%d mismatches" % mism, p,
                   "sum of 'mismatches=' over the hash_check and chain_check lines")


def chain_ns(path, alg):
    """Nanoseconds per chain step, recomputed from the hardware clock counter: best count / (chains * steps) / bus clock."""
    for l in lines_of(path, "RESULT fixcheck chain_time"):
        d = kv(l)
        if d["alg"] == alg:
            allc = [int(x) for x in d["hw_clocks_all"].split("/")]
            best = int(d["hw_clocks_best"])
            assert best == min(allc) and len(allc) == int(d["reps"])
            return best / (int(d["chains"]) * int(d["steps_per_chain"])) / (BUS_MHZ * 1e6) * 1e9, "%d bus clocks / (%s chains x %s steps) / %.0f MHz, best of %d" % (
                best, d["chains"], d["steps_per_chain"], BUS_MHZ, len(allc))
    raise ValueError("no chain_time for %s in %s" % (alg, path))


CH = {}


def chain():
    """Hash-chain step: time, gain of fewer rounds, gain of the split, Amdahl's law"""
    p = "results/fixcheck_rt_n1_450_aup1.txt"
    for alg, letter in (("shake256", "59.7"), ("turboshake256", "33.0"), ("minishake128", "18.4")):
        v, calc = chain_ns(p, alg)
        CH[alg] = v
        check("chain step, %s%s" % (alg, " (per chain; two advance together)" if alg == "minishake128" else ""), letter, v, p, calc, unit=" ns")
    g_r = CH["shake256"] / CH["turboshake256"]
    g_s = CH["turboshake256"] / CH["minishake128"]
    check("gain of halving the rounds on a chain step", "1.81", g_r, p, "%.3f / %.3f" % (CH["shake256"], CH["turboshake256"]), unit="x")
    check("throughput gain of the split on a chain step", "1.80", g_s, p, "%.3f / %.3f" % (CH["turboshake256"], CH["minishake128"]), unit="x")
    clocks = CH["shake256"] * 1e-9 * N_CLOCK[1] * 1e6
    check("clocks per SHAKE256 chain step", "26.9", clocks, p, "%.3f ns x %d MHz" % (CH["shake256"], N_CLOCK[1]))
    f = 24.0 / clocks
    CH["f"] = f
    check("share of a chain step spent in rounds, f", "0.89", f, p, "24 / %.3f" % clocks)
    CH["law_chain"] = 1.0 / ((1 - f) + f / 2)
    check("Amdahl's law for the chain step", "1.81", CH["law_chain"], p, "1 / ((1 - f) + f / 2), f = %.4f" % f, unit="x")
    for fn, algs in (("fixcheck_fix2w_n1_450_aup1.txt", ("shake256", "turboshake256")), ("fixcheck_fix2n_n1_450_aup1.txt", ("minishake128",))):
        for alg in algs:
            v, calc = chain_ns("results/" + fn, alg)
            check_true("fixed-width build gives the same chain-step time, %s" % alg, abs(v - CH[alg]) / CH[alg] < 0.001, "same (%.3f ns)" % CH[alg], "%.3f ns" % v, "results/" + fn, calc)


# ------------------------------------------------------------------------------------------------ 3. Table I
T = {}       # (row, N) -> median in ms, unrounded, as read from the logs


def table():
    """Table I: full applications on N atoms"""
    spec = [("Key generation, SHAKE256", "std", 1, "keygen", ("0.41", "0.27", "0.21")),
            ("Sign, SHAKE256", "std", 1, "sign", ("9.78", "5.56", "3.68")),
            ("Verify, SHAKE256", "std", 1, "verify", ("1.02", "1.00", "1.24")),
            ("Firmware authentication, SHAKE256", "fw", "fw", "verifier_total", ("1.90", "1.90", "2.18")),
            ("Sign, TurboSHAKE256", "turbo_NONSTANDARD", 1, "sign", ("6.81", "3.97", "3.00")),
            ("Sign, MiniSHAKE128 split", "mini_NONSTANDARD", 1, "sign", ("5.41", "3.51", "3.12"))]
    for name, mode, v, op, letter in spec:
        for i, n in enumerate((1, 2, 4)):
            p = "results/sign4_128f_%s_%datoms_aup1.txt" % (mode, n)
            d = time_line(p, v, op)
            x = float(d["median_ms"])
            T[(name, n)] = x
            stats = [float(d[k]) for k in ("min_ms", "q1_ms", "median_ms", "q3_ms", "max_ms")]
            sane = d.get("n") == "25" and stats == sorted(stats) and d.get("wrong", "0") == "0"
            check("%s, N=%d" % (name, n), letter[i], x, p, "'RESULT time v=%s op=%s' median_ms (n=%s%s)" % (v, op, d.get("n"), "" if sane else "; STATISTICS LINE INCONSISTENT"), unit=" ms")
    for i, n in enumerate((1, 2, 4)):
        p = "results/modes_kt_%datoms_aup1.txt" % n
        d = [kv(l) for l in lines_of(p, "RESULT kt ") if kv(l)["mlen"] == "16777216" and kv(l)["atoms"] == str(n)][0]
        x = float(d["median_us"]) / 1000.0
        T[("KT", n)] = x
        T[("KT_proc_share", n)] = float(d["proc_share"])
        check("KangarooTwelve, 16 MiB, N=%d" % n, ("7.68", "7.04", "7.04")[i], x, p, "'RESULT kt mlen=16777216 atoms=%d' median_us / 1000 (n=%s, mismatches=%s)" % (n, d["n"], d["mismatches"]), unit=" ms")
        p = "results/modes_merkle_%datoms_aup1.txt" % n
        for l in lines_of(p, "RESULT merkle k=16"):
            d = kv(l)
            if d["atoms"] == str(n):
                T[("MK_" + d["mode"].split("_")[0], n)] = float(d["median_us"]) / 1000.0
                T[("MKrate_" + d["mode"].split("_")[0], n)] = float(d["Mhash_per_s"])
                T[("MKhashes", n)] = int(d["hashes"])
                if d["mode"].startswith("minishake128"):
                    T[("MK_pack", n)] = float(d["layout_pack_us"]) / 1000.0
                    T[("MK_e2e_mini", n)] = float(d["e2e_with_layout_us"]) / 1000.0
                if d["mode"].startswith("turboshake128"):
                    T[("MK_e2e_turbo", n)] = float(d["e2e_with_layout_us"]) / 1000.0
        check("Merkle tree, 2^16 leaves, MiniSHAKE128 split, N=%d" % n, ("6.28", "3.96", "3.92")[i], T[("MK_minishake128", n)], p,
              "'RESULT merkle k=16 mode=minishake128_split...' median_us / 1000", unit=" ms")
    # second session (run_audit.sh): the same programs with every individual time logged
    def samples(d, field):
        v = sorted(float(x) for x in d[field].split(","))
        return v, v[len(v) // 2]
    def second(claim, letter, d, field, scale, path):
        v, m = samples(d, field)
        printed = float(d["median_" + field.split("_")[1]])
        dev = 100.0 * (m * scale / float(letter) - 1)
        ok = len(v) == 25 and d.get("n") == "25" and abs(m - printed) < 1e-9 and abs(dev) <= 8.0
        add("PASS" if ok else "FAIL", claim + ": second session, median recomputed from the 25 individual times", letter + " ms", "%.4f ms (%+.2f%% from the letter; %d times, %.4f to %.4f ms)" % (
            m * scale, dev, len(v), v[0] * scale, v[-1] * scale), path, "13th of the 25 sorted values of '%s'; it equals the median the program printed; the letter states that times move by up to 8%% between sessions" % field)
    for name, mode, v, op, letter in spec:
        for i, n in enumerate((1, 2, 4)):
            p = "results/sign5_128f_%s_%datoms_aup1.txt" % (mode, n)
            second("%s, N=%d" % (name, n), letter[i], time_line(p, v, op), "samples_ms", 1.0, p)
    for i, n in enumerate((1, 2, 4)):
        p = "results/modes2_kt_%datoms_aup1.txt" % n
        d = [kv(l) for l in lines_of(p, "RESULT kt ") if kv(l)["mlen"] == "16777216" and kv(l)["atoms"] == str(n)][0]
        second("KangarooTwelve, 16 MiB, N=%d" % n, ("7.68", "7.04", "7.04")[i], d, "samples_us", 1e-3, p)
        p = "results/modes2_merkle_%datoms_aup1.txt" % n
        d = [kv(l) for l in lines_of(p, "RESULT merkle k=16") if kv(l)["atoms"] == str(n) and kv(l)["mode"].startswith("minishake128")][0]
        second("Merkle tree, 2^16 leaves, MiniSHAKE128 split, N=%d" % n, ("6.28", "3.96", "3.92")[i], d, "samples_us", 1e-3, p)


# ------------------------------------------------------------------------------------------------ 4. gains and comparisons recomputed from the table
def gains():
    """Gains, limits and comparisons recomputed from the logged medians"""
    S, TU, MI = "Sign, SHAKE256", "Sign, TurboSHAKE256", "Sign, MiniSHAKE128 split"
    src = "results/sign4_128f_{std,turbo,mini}_*atoms_aup1.txt"
    meas = {}
    for i, n in enumerate((1, 2, 4)):
        meas[n] = T[(S, n)] / T[(TU, n)]
        check("measured gain of a TurboSHAKE256 signature over SHAKE256, N=%d" % n, ("1.44", "1.40", "1.23")[i], meas[n], src, "%.4f / %.4f" % (T[(S, n)], T[(TU, n)]), unit="x")
    # the profiling run that gives the share of time the processor is laying out calls
    law = {}
    for i, n in enumerate((1, 2, 4)):
        p = "results/sign_128f_%datoms_aup1.txt" % n
        l = [x for x in lines_of(p, "RESULT breakdown v=1 op=sign")][0]
        total = float(kv(l)["mean_total_ms"]); lay = float(kv(l)["processor_layout_total_ms"])
        check("profiling run, N=%d: processor layout" % n, ("2.18", "2.23", "2.27")[i], lay, p, "'RESULT breakdown v=1 op=sign' processor_layout_total_ms", unit=" ms")
        check("profiling run, N=%d: signature time" % n, ("9.61", "5.55", "3.62")[i], total, p, "same line, mean_total_ms", unit=" ms")
        f = CH["f"] * (1 - lay / total)
        check("share of a signature spent in rounds, f, N=%d" % n, ("0.69", "0.54", "0.33")[i], f, p, "%.4f x (1 - %.4f / %.4f)" % (CH["f"], lay, total))
        law[n] = 1.0 / ((1 - f) + f / 2)
        check("Amdahl's law for the TurboSHAKE256 signature, N=%d" % n, ("1.53", "1.37", "1.20")[i], law[n], p, "1 / ((1 - f) + f / 2), f = %.4f" % f, unit="x")
    check("split signature over TurboSHAKE256, one atom", "1.26", T[(TU, 1)] / T[(MI, 1)], src, "%.4f / %.4f" % (T[(TU, 1)], T[(MI, 1)]), unit="x")
    check("TurboSHAKE256 faster than the split signature on four atoms", "4", 100.0 * (T[(MI, 4)] / T[(TU, 4)] - 1), src, "100 x (%.4f / %.4f - 1)" % (T[(MI, 4)], T[(TU, 4)]), unit="%")
    check_true("one split atom beats two standard atoms", T[(MI, 1)] < T[(S, 2)], "5.41 < 5.56 ms", "%.4f < %.4f ms" % (T[(MI, 1)], T[(S, 2)]), src, "N=1 MiniSHAKE128 sign against N=2 SHAKE256 sign")
    clk = T[(S, 1)] * 1e-3 * N_CLOCK[1] * 1e6
    check("one atom signs in (million clocks of wall time)", "4.40", clk / 1e6, src, "%.4f ms x %d MHz" % (T[(S, 1)], N_CLOCK[1]), unit=" M")
    check("clocks of wall time per hash call, one atom", "42", clk / 104926.0, src, "%.0f clocks / 104,926 hash calls (call count of SLH-DSA-128f as published by Saarinen)" % clk)
    check("LUTs x signing time: Sun et al. ahead of one atom by", "5.8", (10006 * T[(S, 1)]) / (64084 * 0.263), src, "(10,006 LUTs x %.4f ms) / (64,084 LUTs x 0.263 ms as published)" % T[(S, 1)], unit="x")
    # the signing fit t(N) = tp + (ta / N)(450 / F_N), least squares on the three measured points of each family
    worst = 0.0
    for name in (S, TU, MI):
        xs = [(1.0 / n) * (450.0 / N_CLOCK[n]) for n in (1, 2, 4)]; ys = [T[(name, n)] for n in (1, 2, 4)]
        mx, my = sum(xs) / 3, sum(ys) / 3
        ta = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs); tp = my - ta * mx
        worst = max(worst, max(abs(tp + ta * x - y) / y for x, y in zip(xs, ys)))
    check_true("signing fit t(N) = tp + (ta/N)(450/F_N) reproduces the measured times to within", 100 * worst <= 8.0, "8%", "%.2f%% worst" % (100 * worst), src,
               "least squares on N = 1, 2, 4 for SHAKE256, TurboSHAKE256 and MiniSHAKE128; largest |fit - measured| / measured")
    # limits of each application
    for n, letter in ((1, "119"), (4, "206")):
        p = "results/sign4_128f_std_%datoms_aup1.txt" % n
        b = float(kv(lines_of(p, "RESULT breakdown v=1 op=verify")[0])["mean_batches"])
        check("verification batches, N=%d" % n, letter, b, p, "'RESULT breakdown v=1 op=verify' mean_batches")
    p = "results/sign4_128f_std_4atoms_aup1.txt"
    check("verification on two of the four atoms", "1.02", med(p, 1, "verify_on_2_of_4_atoms"), p, "'RESULT time v=1 op=verify_on_2_of_4_atoms' median_ms", unit=" ms")
    d = kv(lines_of(p, "RESULT breakdown v=1 op=sign")[0])
    lay_same = float(d["processor_layout_total_ms"])
    p2 = "results/sign_128f_4atoms_aup1.txt"
    lay_prof = float(kv(lines_of(p2, "RESULT breakdown v=1 op=sign")[0])["processor_layout_total_ms"])
    ok = abs(rnd(lay_same, 2) - 2.35) < 0.005
    add("PASS" if ok else "FAIL", "call layout within the 3.68 ms standard signature at N=4", "2.35 ms", "%.4f ms in the run that gives 3.68 ms" % lay_same, p,
        "'RESULT breakdown v=1 op=sign' processor_layout_total_ms of the same run (the separate profiling run %s has %.4f ms of 3.62 ms)" % (p2, lay_prof))
    p = "results/sign4_128f_mini_NONSTANDARD_4atoms_aup1.txt"
    lay_mini = float(kv(lines_of(p, "RESULT breakdown v=1 op=sign")[0])["processor_layout_total_ms"])
    alt = []
    for fn in ("sign3_128f_mini_NONSTANDARD_word_4atoms_aup1.txt",):
        try:
            l = lines_of("results/" + fn, "RESULT breakdown v=1 op=sign")[0]
            alt.append("%s: %.4f ms layout, median %.4f ms" % (fn, float(kv(l)["processor_layout_total_ms"]), med("results/" + fn, 1, "sign")))
        except Exception:
            pass
    ok = abs(rnd(lay_mini, 2) - 2.73) < 0.005
    add("PASS" if ok else "FAIL", "processor layout within the 3.12 ms split signature at N=4", "2.73 ms", "%.4f ms in the run that gives 3.12 ms" % lay_mini, p,
        "'RESULT breakdown v=1 op=sign' processor_layout_total_ms" + ("; other run: " + "; ".join(alt) if alt else ""))
    p = "results/sign4_128f_tenants_4atoms_aup1.txt"
    rat = []
    for l in lines_of(p, "RESULT tenants split="):
        d = kv(l)
        if "S_ratio" in d:
            rat += [float(d["S_ratio"]), float(d["T_ratio"])]
            assert d["wrong"] == "0"
    check("two tenants slow each other by at most", "1.7", 100 * (max(rat) - 1), p, "largest of S_ratio and T_ratio over the three splits, minus 1 (medians of n=25, shared / alone)", unit="%")
    p = "results/sign4_128f_fw_1atoms_aup1.txt"
    check("of the 1.90 ms firmware authentication, hashing 1 MiB", "0.88", med(p, "fw", "verifier_hash_1MB"), p, "'RESULT time v=fw op=verifier_hash_1MB' median_ms", unit=" ms")
    return meas, law


def kt_and_tree(meas, law):
    """KangarooTwelve and the Merkle tree: rates, limits, software baselines, forecasts"""
    bits = 16777216 * 8
    gb = {n: bits / (T[("KT", n)] * 1e-3) / 1e9 for n in (1, 2, 4)}
    src = "results/modes_kt_*atoms_aup1.txt"
    check("KangarooTwelve on one atom", "17.5", gb[1], src, "16,777,216 bytes x 8 / %.4f ms" % T[("KT", 1)], unit=" Gb/s")
    check("KangarooTwelve on two or four atoms", "19.1", gb[2], src, "16,777,216 bytes x 8 / %.4f ms" % T[("KT", 2)], unit=" Gb/s")
    check("the same on four atoms", "19.1", gb[4], src, "16,777,216 bytes x 8 / %.4f ms" % T[("KT", 4)], unit=" Gb/s")
    port = 64 * BUS_MHZ * 1e6 / 1e9
    check("memory port: 64 bits at the bus clock", "19.2", port, "synthesis/reports/atom_v3_n1_m16_300_450/route_timing.rpt", "64 bits x %.0f MHz" % BUS_MHZ, unit=" Gb/s")
    check("share of the port on two atoms", "99", 100 * gb[2] / port, src, "%.3f / %.1f" % (gb[2], port), unit="%")
    check("processor busy during KangarooTwelve", "0.2", 100 * T[("KT_proc_share", 2)], src, "'RESULT kt mlen=16777216 atoms=2' proc_share", unit="%")
    for n in (1, 2, 4):
        p = "results/modes_kt_%datoms_aup1.txt" % n
        ok = {}
        for l in lines_of(p, "RESULT ktvec"):
            d = kv(l)
            if "idx" not in d:
                continue
            ok.setdefault(d["atoms"], set())
            if d.get("sw_ok") == "true" and d.get("atoms_ok") == "true":
                ok[d["atoms"]].add(d["idx"])
        allok = all(len(v) == 18 for v in ok.values()) and len(ok) >= 1
        check_true("RFC 9861 KangarooTwelve vectors reproduced on the %d-atom build" % n, allok, "18", " ".join("%s atom(s): %d" % (k, len(v)) for k, v in sorted(ok.items())), p,
                   "'RESULT ktvec' lines with sw_ok=true and atoms_ok=true, distinct idx, for each number of atoms used")
    # the board's own processor cores, plain C
    p = "results/modes_sw_a53_aup1.txt"
    sw = {}
    for l in lines_of(p, "RESULT swkt"):
        d = kv(l)
        if d["mlen"] == "16777216":
            sw[int(d["threads"])] = bits / (float(d["median_us"]) * 1e-6) / 1e9
    check("KangarooTwelve on the four Cortex-A53 cores of the board", "5.4", sw[4], p, "'RESULT swkt mlen=16777216 threads=4': 16,777,216 x 8 / median_us", unit=" Gb/s")
    check("four atoms over four cores", "3.5", gb[4] / sw[4], src + ", " + p, "%.3f / %.3f" % (gb[4], sw[4]), unit="x")
    p = "results/modes_kt_4atoms_aup1.txt"
    flat = {}
    for l in lines_of(p, "RESULT flat"):
        d = kv(l)
        if int(d["mlen"]) >= 16777208:
            flat[d["alg"]] = float(d["median_us"]) / 1000.0
    check("KangarooTwelve on four atoms over one flat TurboSHAKE128 sponge", "1.21", flat["turboshake128"] / T[("KT", 4)], p, "%.4f ms / %.4f ms" % (flat["turboshake128"], T[("KT", 4)]), unit="x")
    check("and over flat SHAKE128", "1.63", flat["shake128"] / T[("KT", 4)], p, "%.4f ms / %.4f ms" % (flat["shake128"], T[("KT", 4)]), unit="x")
    # the Merkle tree
    src = "results/modes_merkle_*atoms_aup1.txt"
    g = {n: T[("MK_turboshake128", n)] / T[("MK_minishake128", n)] for n in (1, 2, 4)}
    for i, n in enumerate((1, 2, 4)):
        check("Merkle tree: split over TurboSHAKE128 on the wide atom, N=%d" % n, ("1.35", "1.16", "1.01")[i], g[n], src, "%.4f / %.4f ms" % (T[("MK_turboshake128", n)], T[("MK_minishake128", n)]), unit="x")
    rates = [T[("MKrate_" + m, 4)] for m in ("minishake128", "turboshake128", "shake128")]
    ok = all(rnd(r, 0) in (32.0, 33.0) for r in rates)
    add("PASS" if ok else "FAIL", "at N=4 the three functions reach 32 to 33 million hashes per second", "32 to 33", "%.2f (MiniSHAKE128), %.2f (TurboSHAKE128), %.2f (SHAKE128)" % tuple(rates),
        "results/modes_merkle_4atoms_aup1.txt", "'RESULT merkle k=16 ... atoms=4' Mhash_per_s of each of the three modes")
    check("interleaving the leaf values for the split layout", "1.7", T[("MK_pack", 1)], "results/modes_merkle_1atoms_aup1.txt", "'RESULT merkle k=16 mode=minishake128...' layout_pack_us / 1000", unit=" ms")
    wins = [n for n in (1, 2, 4) if T[("MK_e2e_mini", n)] < T[("MK_e2e_turbo", n)]]
    check_true("with the interleave counted, the split wins only at N=1", wins == [1], "N=1 only", "split wins at N=%s" % wins, src,
               "e2e_with_layout_us of the split tree against the TurboSHAKE128 tree: " + ", ".join("N=%d %.3f vs %.3f ms" % (n, T[("MK_e2e_mini", n)], T[("MK_e2e_turbo", n)]) for n in (1, 2, 4)))
    p = "results/modes_xkcp_sw_a53_aup1.txt"
    x = {}
    for l in lines_of(p, "RESULT xkcpmerkle k=16"):
        d = kv(l)
        x[(d["alg"], int(d["threads"]))] = float(d["median_us"]) / 1000.0
    check("the designers' C code builds the tree on four Cortex-A53 cores in", "33.5", x[("minishake128", 4)], p, "'RESULT xkcpmerkle k=16 alg=minishake128 threads=4' median_us / 1000", unit=" ms")
    check("that is behind four atoms by", "8.6", x[("minishake128", 4)] / T[("MK_minishake128", 4)], p + ", " + src, "%.3f / %.4f ms" % (x[("minishake128", 4)], T[("MK_minishake128", 4)]), unit="x")
    check("in that code the narrow state takes this share of the 1600-bit time", "0.97", x[("minishake128", 1)] / x[("turboshake128", 1)], p,
          "%.3f / %.3f ms, one core" % (x[("minishake128", 1)], x[("turboshake128", 1)]))
    # the forecasts of the law for the tree, as filed, and the accuracy the abstract states
    fp = "results/forecast_modes_2026-11-26.txt"
    ft = read(fp)
    fc = [float(re.search(r"N = %d:.*?G = ([0-9.]+)" % n, ft).group(1)) for n in (1, 2, 4)]
    check_true("the law's forecast for the tree, as filed before the run", fc == [1.25, 1.10, 1.00], "1.25, 1.10, 1.00", ", ".join("%.2f" % v for v in fc), fp, "section 5 of the forecast file, lines 'N = 1/2/4: ... G ='")
    dev = [("chain step", CH["law_chain"], CH["shake256"] / CH["turboshake256"])] + [("signature N=%d" % n, law[n], meas[n]) for n in (1, 2, 4)] + [("tree N=%d" % n, fc[i], g[n]) for i, n in enumerate((1, 2, 4))]
    worst = max(dev, key=lambda t: abs(t[1] - t[2]) / t[2])
    check_true("Amdahl's law predicts the gains of a chain, a signature and a tree to within", abs(worst[1] - worst[2]) / worst[2] < 0.08, "8%",
               "%.2f%% worst (%s: law %.3f, measured %.3f)" % (100 * abs(worst[1] - worst[2]) / worst[2], worst[0], worst[1], worst[2]), "the checks above",
               "largest |law - measured| / measured over: " + "; ".join("%s %.3f vs %.3f" % t for t in dev))
    # published figures of Karl et al., arithmetic only
    f = 24.0 / (42.6e6 / 104926.0)
    check("Karl et al.: cycles per hash call", "406", 42.6e6 / 104926.0, "published values (not in this repository)", "42.6 million cycles / 104,926 calls", mandatory=False)
    check("the law for that share", "3.0", 100 * (1.0 / ((1 - f) + f / 2) - 1), "published values (not in this repository)", "f = 24 / 406; 1 / ((1 - f) + f / 2) - 1", mandatory=False, unit="%")


# ------------------------------------------------------------------------------------------------ 5. the model and the forecasts
def model():
    """Simulator against the board, design decisions, forecast errors"""
    # board times of the job-graph runs, read from the logs (best of 7, field ms=)
    def dfg(path, nocopy):
        out = {}; mode = None
        for l in lines_of(path, "RESULT dfg"):
            if " mode no_copy=" in l:
                mode = l.strip().endswith("true")
            m = re.search(r"mix=trees22\+tags(\d+)x2400\+fw(\d+)x1MB policy=(\S+) atoms=(\d+) ms=([0-9.]+)", l)
            if m and (mode is None or mode == nocopy):
                out[(int(m.group(4)), int(m.group(1)), int(m.group(2)), m.group(3))] = float(m.group(5))
        return out
    sys.path.insert(0, SIM)
    for need in ("dfg_sim.py", "dfg_sim_gem5.py", "gem5_costs.json"):
        if not os.path.isfile(os.path.join(SIM, need)):
            raise Missing("model/sim/" + need)
    import dfg_sim, dfg_sim_gem5 as g
    pol = {"serial": 0, "graph": 1, "graph+rule": 2}
    meas36 = {}
    for n, f in ((4, "atom_dfg_n4_400_aup1.txt"), (2, "atom_dfg_n2_420_aup1.txt")):
        for (a, tg, fw, p), ms in dfg("results/" + f, False).items():
            meas36.setdefault((a, N_CLOCK[a], tg, fw), [None] * 3)[pol[p]] = ms
    nocopy = {}; withcopy = {}
    for n, f in ((4, "atom_dfg2_nocopy_n4_400_aup1.txt"), (2, "atom_dfg2_nocopy_n2_420_aup1.txt")):
        for (a, tg, fw, p), ms in dfg("results/" + f, True).items():
            if p == "graph+rule" and (tg or fw):
                nocopy[(a, N_CLOCK[a], tg, fw)] = ms
        for (a, tg, fw, p), ms in dfg("results/" + f, False).items():
            if p == "graph+rule" and (tg or fw):
                withcopy[(a, N_CLOCK[a], tg, fw)] = ms
    src = "results/atom_dfg_n{4_400,2_420}_aup1.txt"
    check_true("board times the simulator is compared with are the logged ones (36 with copy)", meas36 == {k: list(v) for k, v in dfg_sim.MEAS.items()}, "36 equal", "%d runs read, %s" % (
        sum(len(v) for v in meas36.values()), "all equal to model/sim/dfg_sim.py" if meas36 == {k: list(v) for k, v in dfg_sim.MEAS.items()} else "DIFFERENT from model/sim/dfg_sim.py"), src, "field ms= (best of 7) of every 'RESULT dfg mix=' line")
    check_true("and the 10 no-copy rows", nocopy == g.NOCOPY, "10 equal", "%d rows read, %s" % (len(nocopy), "all equal to model/sim/dfg_sim_gem5.py" if nocopy == g.NOCOPY else "DIFFERENT"),
               "results/atom_dfg2_nocopy_n{4_400,2_420}_aup1.txt", "section 'mode no_copy=true', policy graph+rule")
    # processor costs: the gem5 numbers in gem5_costs.json must be the ones gem5 printed, and the board ones the ones the board printed
    import json
    C = json.load(open(os.path.join(SIM, "gem5_costs.json")))
    def cost_file(path):
        return {m.group(1): int(m.group(2)) for m in re.finditer(r"COST (\S+) min_ns=\d+ mean_ns=(\d+)", read(path))}
    gp = "model/gem5_atom/calib/r4_pflat4.txt"; gm = cost_file(gp)
    check_true("gem5 processor costs used by the simulator are the output of the frozen gem5 run", all(gm[k] == v for k, v in C["gem5"].items()), "%d routines equal" % len(C["gem5"]),
               "%d of %d equal" % (sum(1 for k, v in C["gem5"].items() if gm.get(k) == v), len(C["gem5"])), gp + " against model/sim/gem5_costs.json", "mean_ns of each 'COST' line")
    bp, bf = "results/cost_native_a53_noflush_aup1.txt", "results/cost_native_a53_aup1.txt"
    b0, b1 = cost_file(bp), cost_file(bf)
    check_true("cache-flush costs used by the simulator are board measurements", all(b1[k] - b0[k] == v for k, v in C["flush_board_routine"].items()), "%d routines equal" % len(C["flush_board_routine"]),
               "%d of %d equal" % (sum(1 for k, v in C["flush_board_routine"].items() if b1[k] - b0[k] == v), len(C["flush_board_routine"])), bf + " - " + bp, "mean_ns with flush minus mean_ns without")
    # run the simulator with gem5 costs and nothing fitted, against the board times read above
    p = g.PG("gem5", "routine")
    e36 = [(g.simulate(k[0], k[1], i, k[2], k[3], p)["ms"] - v[i]) / v[i] for k, v in meas36.items() for i in range(3)]
    e10 = [(g.simulate(k[0], k[1], 2, k[2], k[3], p, zero_copy=0.0)["ms"] - v) / v for k, v in nocopy.items()]
    e = [abs(x) for x in e36 + e10]
    check_true("number of board runs the simulator is checked on", len(e) == 46, "46", len(e), src + " and the no-copy logs", "36 with copy + 10 without")
    check("simulator against the board, mean error", "4.9", 100 * sum(e) / len(e), "model/sim/dfg_sim_gem5.py run on the logged board times", "mean |simulated - board| / board over the 46 runs; 36 runs %.2f%%, 10 runs %.2f%%" % (
        100 * sum(abs(x) for x in e36) / 36, 100 * sum(abs(x) for x in e10) / 10), unit="%")
    check("simulator against the board, worst error", "9.5", 100 * max(e), "model/sim/dfg_sim_gem5.py run on the logged board times", "largest |simulated - board| / board over the 46 runs", unit="%")
    # the design decisions
    fast = sorted(100 * (withcopy[k] - nocopy[k]) / withcopy[k] for k in nocopy if k[0] == 4)
    check_true("reading in place made mixed jobs faster on the board by", rnd(fast[0], 0) == 31 and rnd(fast[-1], 0) == 43, "31 to 43%", "%.1f to %.1f%%" % (fast[0], fast[-1]),
               "results/atom_dfg2_nocopy_n4_400_aup1.txt", "four atoms, policy graph+rule, same run: (with copy - no copy) / with copy over the five mixed jobs: " + ", ".join("%.1f" % v for v in fast))
    rk = read("model/sim/design_ranking.txt")
    blocks = re.split(r"\n mix tags=", "\n" + rk.split("Atom count beyond")[0])[1:]
    names = set(); sec = []
    for b in blocks:
        for m in re.finditer(r"^\s{4}(\S.*?)\s{2,}[0-9.]+ \(([+-][0-9.]+)%\)", b, re.M):
            names.add(m.group(1).strip())
            if m.group(1).strip() == "secrets+chains as one hardware node":
                sec.append(-float(m.group(2)))
    check_true("scheduler designs ranked by the model", len(names) == 13, "13", len(names), "model/sim/design_ranking.txt", "distinct design names in the ranking")
    check_true("deriving the secret inside the chain step would save", rnd(min(sec), 0) == 2 and rnd(max(sec), 0) == 6, "2 to 6%", "%.1f to %.1f%%" % (min(sec), max(sec)), "model/sim/design_ranking.txt",
               "rows 'secrets+chains as one hardware node' over the job mixes")
    # the forecast that failed
    fp = "results/forecast_sign3_split_layout_2026-11-26.txt"
    ft = read(fp); errs = []
    for mode in ("mini", "hybrid"):
        m = re.search(r"^\s*%s\s+1 atom @450: ([0-9.]+).*?2 atoms @420: ([0-9.]+).*?4 atoms @400: ([0-9.]+)" % mode, ft, re.M)
        for i, n in enumerate((1, 2, 4)):
            fc = float(m.group(i + 1)); got = med("results/sign3_128f_%s_NONSTANDARD_word_%datoms_aup1.txt" % (mode, n), 1, "sign")
            errs.append(100 * (got / fc - 1))
    check_true("the simulator's forecast of the split signature failed, optimistically, by", rnd(min(errs), 0) == 26 and rnd(max(errs), 0) == 77 and min(errs) > 0, "26 to 77%",
               "%.1f to %.1f%%" % (min(errs), max(errs)), fp + " against results/sign3_128f_{mini,hybrid}_NONSTANDARD_word_*atoms_aup1.txt",
               "measured median / forecast - 1 for the six rows: " + ", ".join("%+.1f" % v for v in errs))
    # area forecast and the order of forecast and run, from the modification times recorded in the manifest
    man = {}
    for l in read("MANIFEST.sha256").splitlines():
        c = l.split()
        if len(c) == 4 and not l.startswith("#"):
            man[c[3]] = c[2]
    pairs = [("results/forecast_modes_2026-11-26.txt", "results/modes_kt_1atoms_aup1.txt"), ("results/forecast_modes_2026-11-26.txt", "results/modes_merkle_1atoms_aup1.txt"),
             ("results/forecast_sign3_split_layout_2026-11-26.txt", "results/sign3_128f_mini_NONSTANDARD_word_1atoms_aup1.txt"),
             ("results/forecast_sign_2026-11-26.txt", "results/sign_128f_1atoms_aup1.txt")]
    for a, b in pairs:
        if a not in man or b not in man:
            raise Missing("MANIFEST.sha256 entry for %s or %s" % (a, b))
        check_true("forecast file older than the first log of its run: %s" % os.path.basename(a), man[a] < man[b], "forecast first", "%s < %s" % (man[a], man[b]), "MANIFEST.sha256",
                   "modification times (UTC) recorded when the release was staged: %s against %s" % (a, b), mandatory=False)


# ------------------------------------------------------------------------------------------------ run and report
def main():
    section("1. Cost of the second width"); guard(cost)
    section("2. Correctness"); guard(vectors); guard(fips205); guard(width_changes)
    section("3. Hash-chain step and Amdahl's law"); guard(chain)
    section("4. Table I"); guard(table)
    section("5. Gains and limits")
    res = [None]
    def _g():
        """Gains, limits and comparisons recomputed from the logged medians"""
        res[0] = gains()
    guard(_g)
    def _k():
        """KangarooTwelve and the Merkle tree: rates, limits, software baselines, forecasts"""
        kt_and_tree(*res[0])
    if res[0]:
        guard(_k)
    else:
        add("MISSING", "KangarooTwelve and the Merkle tree", "-", "-", "-", "the group above did not complete")
    section("6. Model and forecasts"); guard(model)

    cur = None
    for sec, st, mand, claim, letter, got, source, calc in rows:
        if QUIET and st == "PASS":
            continue
        if sec != cur:
            print("\n== %s" % sec); cur = sec
        print("%-7s %s%s" % (st, claim, "" if mand else "  (info)"))
        print("          letter: %s | recomputed: %s" % (letter, got))
        print("          source: %s" % source)
        if calc:
            print("          how:    %s" % calc)
    mand = [r for r in rows if r[2]]
    npass = sum(1 for r in mand if r[1] == "PASS"); nfail = sum(1 for r in mand if r[1] == "FAIL"); nmiss = sum(1 for r in mand if r[1] == "MISSING")
    info = [r for r in rows if not r[2]]
    print("\n== Summary")
    print("mandatory checks: %d   PASS %d   FAIL %d   MISSING %d" % (len(mand), npass, nfail, nmiss))
    print("informational checks: %d   PASS %d   other %d" % (len(info), sum(1 for r in info if r[1] == "PASS"), sum(1 for r in info if r[1] != "PASS")))
    if nfail or nmiss:
        print("\nDiscrepancies:")
        for sec, st, m, claim, letter, got, source, calc in rows:
            if m and st != "PASS":
                print("  %s  %s\n      letter: %s | recomputed: %s\n      source: %s\n      %s" % (st, claim, letter, got, source, calc))
    print("\nNot covered by this script (no log in the repository can decide them): the count of 754 one-bit multiplexers (10, 712, 32), which is a property of rtl/atom/atom_seq3.v;")
    print("the figures of other authors (SLotH, SPHINCSLET, Trident, Sun et al., Karl et al., the Keccak team's desktop figures), which are quoted from their papers.")
    sys.exit(1 if (nfail or nmiss) else 0)


if __name__ == "__main__":
    main()
