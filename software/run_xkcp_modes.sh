#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# run_xkcp_modes.sh (NEW): XKCP-permutation software comparator for the Merkle trees. Runs AFTER run_modes.sh (needs its logs).
# Gate: XKCP MiniSHAKE128 digests must equal our two MiniSHAKE implementations (Rust: ruty_apps refvec + ruty_modes sw_root; C: keccak_bench hex
# + modes_bench roots) on the same inputs. If any differ, nothing is timed.
cd ~/Ruty/apps || exit 1
REPS=${1:-25}; KS=${2:-12,16,20}; X=$HOME/Ruty/apps/xkcp_fa7033a/lib
I="-I$X/common -I$X/low/common -I$X/low/KeccakP-800/plain -I$X/low/KeccakP-800/plain/ua -I$X/low/KeccakP-1600/plain-64bits -I$X/low/KeccakP-1600/plain-64bits/ua -I$X/low/KeccakP-1600/common"
sed -i 's/\r$//' xkcp_modes_bench.c
echo "XSTART $(date +%H:%M:%S) xkcp commit fa7033a212c831249b7e17e785ee1a4e24fc04c4"
rm -f xkcp_modes_bench xk_p800.o xk_p1600.o
gcc -O3 -mcpu=cortex-a53 $I -c $X/low/KeccakP-800/plain/KeccakP-800-opt32.c -o xk_p800.o && gcc -O3 -mcpu=cortex-a53 $I -c $X/low/KeccakP-1600/plain-64bits/KeccakP-1600-opt64.c -o xk_p1600.o \
 && gcc -O3 -mcpu=cortex-a53 -pthread $I -o xkcp_modes_bench xkcp_modes_bench.c xk_p800.o xk_p1600.o
[ -x xkcp_modes_bench ] || { echo "XKCP BUILD FAILED"; exit 1; }
./xkcp_modes_bench digests > modes_xkcp_digests.log; head -1 modes_xkcp_digests.log
BAD=0
A=$(grep -m1 'RESULT refvec minishake128 dom=01' apps_atom_v3_n1_450.log | grep -o 'out16=[0-9a-f]*'); B=$(grep -m1 'xdig refvec' modes_xkcp_digests.log | grep -o 'out16=[0-9a-f]*')
echo "GATE refvec rust_ruty_apps=$A xkcp=$B"; [ -n "$A" ] && [ "$A" == "$B" ] || BAD=1
for alg in minishake128 turboshake128 shake128; do for L in 0 1 32 63 64 67 68 69 100 136 1000; do
  C=$(./keccak_bench $alg $L 32 hex); XD=$(grep "kbhex $alg dom=1f len=$L " modes_xkcp_digests.log | grep -o 'out32=[0-9a-f]*' | cut -d= -f2)
  if [ "$C" != "$XD" ]; then echo "GATE MISMATCH $alg len=$L keccak_bench=$C xkcp=$XD"; BAD=1; fi; done; done
echo "GATE keccak_bench(C, ours) vs xkcp: 33 digests checked, bad=$BAD"
./xkcp_modes_bench merkle 1 1 $KS > modes_xkcp_roots.log
for k in ${KS//,/ }; do for alg in minishake128 turboshake128 shake128; do
  XR=$(grep "xkcpmerkle k=$k alg=$alg " modes_xkcp_roots.log | grep -o 'root=[0-9a-f]*' | cut -d= -f2)
  CR=$(grep "swmerkle k=$k alg=$alg threads=1 " modes_sw.log | grep -o 'root=[0-9a-f]*' | cut -d= -f2)
  RR=$(grep "merkle_root k=$k " modes_merkle_atom_v3_n1_450.log | grep "alg=$alg " | grep -o 'sw_root=[0-9a-f]*' | cut -d= -f2)
  if [ -z "$XR" ] || [ "$XR" != "$CR" ] || [ "$XR" != "$RR" ]; then echo "GATE ROOT MISMATCH k=$k $alg xkcp=$XR modes_bench=$CR ruty_modes_rust=$RR"; BAD=1; else echo "GATE root k=$k $alg agree (xkcp = modes_bench C = ruty_modes Rust)"; fi; done; done
if [ $BAD -ne 0 ]; then echo "XKCP GATE FAILED: NOT TIMED"; exit 1; fi
echo "XKCP GATE PASSED $(date +%H:%M:%S)"
{ ./xkcp_modes_bench merkle $REPS 1 $KS; ./xkcp_modes_bench merkle $REPS 4 $KS; } > modes_xkcp_sw.log 2>&1
grep xkcpmerkle modes_xkcp_sw.log | cut -c1-200
echo "XDONE $(date +%H:%M:%S)"
