// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
/* ruty_gem5/sw/keccak_bench.c : plain-C software baseline for the RUTY study.
   One file, no dependencies. Portable C with the round fully unrolled on 25 local lanes (not hand-optimized assembly): say so when quoting its numbers.
   usage: keccak_bench <alg> <msglen> <outlen> <count>      -> prints RESULT line with ns per hash
          keccak_bench <alg> <msglen> <outlen> hex          -> prints the digest of bytes 0,1,2,... (self-check)
   alg: shake128 shake256 turboshake128 turboshake256 minishake128 minishake256                              */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
static const uint64_t RC[24] = {0x0000000000000001ULL,0x0000000000008082ULL,0x800000000000808aULL,0x8000000080008000ULL,0x000000000000808bULL,0x0000000080000001ULL,0x8000000080008081ULL,0x8000000000008009ULL,0x000000000000008aULL,0x0000000000000088ULL,0x0000000080008009ULL,0x000000008000000aULL,0x000000008000808bULL,0x800000000000008bULL,0x8000000000008089ULL,0x8000000000008003ULL,0x8000000000008002ULL,0x8000000000000080ULL,0x000000000000800aULL,0x800000008000000aULL,0x8000000080008081ULL,0x8000000000008080ULL,0x0000000080000001ULL,0x8000000080008008ULL};
static void perm64(uint64_t *A, int first, int last) {
  uint64_t a0=A[0], a1=A[1], a2=A[2], a3=A[3], a4=A[4], a5=A[5], a6=A[6], a7=A[7], a8=A[8], a9=A[9], a10=A[10], a11=A[11], a12=A[12], a13=A[13], a14=A[14], a15=A[15], a16=A[16], a17=A[17], a18=A[18], a19=A[19], a20=A[20], a21=A[21], a22=A[22], a23=A[23], a24=A[24];
  uint64_t b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15, b16, b17, b18, b19, b20, b21, b22, b23, b24, c0, c1, c2, c3, c4, d0, d1, d2, d3, d4; int r;
  for (r = first; r <= last; r++) {
    c0 = a0 ^ a5 ^ a10 ^ a15 ^ a20;
    c1 = a1 ^ a6 ^ a11 ^ a16 ^ a21;
    c2 = a2 ^ a7 ^ a12 ^ a17 ^ a22;
    c3 = a3 ^ a8 ^ a13 ^ a18 ^ a23;
    c4 = a4 ^ a9 ^ a14 ^ a19 ^ a24;
    d0 = c4 ^ (uint64_t)((c1 << 1) | (c1 >> 63));
    d1 = c0 ^ (uint64_t)((c2 << 1) | (c2 >> 63));
    d2 = c1 ^ (uint64_t)((c3 << 1) | (c3 >> 63));
    d3 = c2 ^ (uint64_t)((c4 << 1) | (c4 >> 63));
    d4 = c3 ^ (uint64_t)((c0 << 1) | (c0 >> 63));
    b0 = (a0 ^ d0);
    { uint64_t t = (a5 ^ d0); b16 = (uint64_t)((t << 36) | (t >> 28)); }
    { uint64_t t = (a10 ^ d0); b7 = (uint64_t)((t << 3) | (t >> 61)); }
    { uint64_t t = (a15 ^ d0); b23 = (uint64_t)((t << 41) | (t >> 23)); }
    { uint64_t t = (a20 ^ d0); b14 = (uint64_t)((t << 18) | (t >> 46)); }
    { uint64_t t = (a1 ^ d1); b10 = (uint64_t)((t << 1) | (t >> 63)); }
    { uint64_t t = (a6 ^ d1); b1 = (uint64_t)((t << 44) | (t >> 20)); }
    { uint64_t t = (a11 ^ d1); b17 = (uint64_t)((t << 10) | (t >> 54)); }
    { uint64_t t = (a16 ^ d1); b8 = (uint64_t)((t << 45) | (t >> 19)); }
    { uint64_t t = (a21 ^ d1); b24 = (uint64_t)((t << 2) | (t >> 62)); }
    { uint64_t t = (a2 ^ d2); b20 = (uint64_t)((t << 62) | (t >> 2)); }
    { uint64_t t = (a7 ^ d2); b11 = (uint64_t)((t << 6) | (t >> 58)); }
    { uint64_t t = (a12 ^ d2); b2 = (uint64_t)((t << 43) | (t >> 21)); }
    { uint64_t t = (a17 ^ d2); b18 = (uint64_t)((t << 15) | (t >> 49)); }
    { uint64_t t = (a22 ^ d2); b9 = (uint64_t)((t << 61) | (t >> 3)); }
    { uint64_t t = (a3 ^ d3); b5 = (uint64_t)((t << 28) | (t >> 36)); }
    { uint64_t t = (a8 ^ d3); b21 = (uint64_t)((t << 55) | (t >> 9)); }
    { uint64_t t = (a13 ^ d3); b12 = (uint64_t)((t << 25) | (t >> 39)); }
    { uint64_t t = (a18 ^ d3); b3 = (uint64_t)((t << 21) | (t >> 43)); }
    { uint64_t t = (a23 ^ d3); b19 = (uint64_t)((t << 56) | (t >> 8)); }
    { uint64_t t = (a4 ^ d4); b15 = (uint64_t)((t << 27) | (t >> 37)); }
    { uint64_t t = (a9 ^ d4); b6 = (uint64_t)((t << 20) | (t >> 44)); }
    { uint64_t t = (a14 ^ d4); b22 = (uint64_t)((t << 39) | (t >> 25)); }
    { uint64_t t = (a19 ^ d4); b13 = (uint64_t)((t << 8) | (t >> 56)); }
    { uint64_t t = (a24 ^ d4); b4 = (uint64_t)((t << 14) | (t >> 50)); }
    a0 = b0 ^ (~b1 & b2);
    a1 = b1 ^ (~b2 & b3);
    a2 = b2 ^ (~b3 & b4);
    a3 = b3 ^ (~b4 & b0);
    a4 = b4 ^ (~b0 & b1);
    a5 = b5 ^ (~b6 & b7);
    a6 = b6 ^ (~b7 & b8);
    a7 = b7 ^ (~b8 & b9);
    a8 = b8 ^ (~b9 & b5);
    a9 = b9 ^ (~b5 & b6);
    a10 = b10 ^ (~b11 & b12);
    a11 = b11 ^ (~b12 & b13);
    a12 = b12 ^ (~b13 & b14);
    a13 = b13 ^ (~b14 & b10);
    a14 = b14 ^ (~b10 & b11);
    a15 = b15 ^ (~b16 & b17);
    a16 = b16 ^ (~b17 & b18);
    a17 = b17 ^ (~b18 & b19);
    a18 = b18 ^ (~b19 & b15);
    a19 = b19 ^ (~b15 & b16);
    a20 = b20 ^ (~b21 & b22);
    a21 = b21 ^ (~b22 & b23);
    a22 = b22 ^ (~b23 & b24);
    a23 = b23 ^ (~b24 & b20);
    a24 = b24 ^ (~b20 & b21);
    a0 ^= (uint64_t)RC[r];
  }
  A[0] = a0;
  A[1] = a1;
  A[2] = a2;
  A[3] = a3;
  A[4] = a4;
  A[5] = a5;
  A[6] = a6;
  A[7] = a7;
  A[8] = a8;
  A[9] = a9;
  A[10] = a10;
  A[11] = a11;
  A[12] = a12;
  A[13] = a13;
  A[14] = a14;
  A[15] = a15;
  A[16] = a16;
  A[17] = a17;
  A[18] = a18;
  A[19] = a19;
  A[20] = a20;
  A[21] = a21;
  A[22] = a22;
  A[23] = a23;
  A[24] = a24;
}
static void perm32(uint32_t *A, int first, int last) {
  uint32_t a0=A[0], a1=A[1], a2=A[2], a3=A[3], a4=A[4], a5=A[5], a6=A[6], a7=A[7], a8=A[8], a9=A[9], a10=A[10], a11=A[11], a12=A[12], a13=A[13], a14=A[14], a15=A[15], a16=A[16], a17=A[17], a18=A[18], a19=A[19], a20=A[20], a21=A[21], a22=A[22], a23=A[23], a24=A[24];
  uint32_t b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15, b16, b17, b18, b19, b20, b21, b22, b23, b24, c0, c1, c2, c3, c4, d0, d1, d2, d3, d4; int r;
  for (r = first; r <= last; r++) {
    c0 = a0 ^ a5 ^ a10 ^ a15 ^ a20;
    c1 = a1 ^ a6 ^ a11 ^ a16 ^ a21;
    c2 = a2 ^ a7 ^ a12 ^ a17 ^ a22;
    c3 = a3 ^ a8 ^ a13 ^ a18 ^ a23;
    c4 = a4 ^ a9 ^ a14 ^ a19 ^ a24;
    d0 = c4 ^ (uint32_t)((c1 << 1) | (c1 >> 31));
    d1 = c0 ^ (uint32_t)((c2 << 1) | (c2 >> 31));
    d2 = c1 ^ (uint32_t)((c3 << 1) | (c3 >> 31));
    d3 = c2 ^ (uint32_t)((c4 << 1) | (c4 >> 31));
    d4 = c3 ^ (uint32_t)((c0 << 1) | (c0 >> 31));
    b0 = (a0 ^ d0);
    { uint32_t t = (a5 ^ d0); b16 = (uint32_t)((t << 4) | (t >> 28)); }
    { uint32_t t = (a10 ^ d0); b7 = (uint32_t)((t << 3) | (t >> 29)); }
    { uint32_t t = (a15 ^ d0); b23 = (uint32_t)((t << 9) | (t >> 23)); }
    { uint32_t t = (a20 ^ d0); b14 = (uint32_t)((t << 18) | (t >> 14)); }
    { uint32_t t = (a1 ^ d1); b10 = (uint32_t)((t << 1) | (t >> 31)); }
    { uint32_t t = (a6 ^ d1); b1 = (uint32_t)((t << 12) | (t >> 20)); }
    { uint32_t t = (a11 ^ d1); b17 = (uint32_t)((t << 10) | (t >> 22)); }
    { uint32_t t = (a16 ^ d1); b8 = (uint32_t)((t << 13) | (t >> 19)); }
    { uint32_t t = (a21 ^ d1); b24 = (uint32_t)((t << 2) | (t >> 30)); }
    { uint32_t t = (a2 ^ d2); b20 = (uint32_t)((t << 30) | (t >> 2)); }
    { uint32_t t = (a7 ^ d2); b11 = (uint32_t)((t << 6) | (t >> 26)); }
    { uint32_t t = (a12 ^ d2); b2 = (uint32_t)((t << 11) | (t >> 21)); }
    { uint32_t t = (a17 ^ d2); b18 = (uint32_t)((t << 15) | (t >> 17)); }
    { uint32_t t = (a22 ^ d2); b9 = (uint32_t)((t << 29) | (t >> 3)); }
    { uint32_t t = (a3 ^ d3); b5 = (uint32_t)((t << 28) | (t >> 4)); }
    { uint32_t t = (a8 ^ d3); b21 = (uint32_t)((t << 23) | (t >> 9)); }
    { uint32_t t = (a13 ^ d3); b12 = (uint32_t)((t << 25) | (t >> 7)); }
    { uint32_t t = (a18 ^ d3); b3 = (uint32_t)((t << 21) | (t >> 11)); }
    { uint32_t t = (a23 ^ d3); b19 = (uint32_t)((t << 24) | (t >> 8)); }
    { uint32_t t = (a4 ^ d4); b15 = (uint32_t)((t << 27) | (t >> 5)); }
    { uint32_t t = (a9 ^ d4); b6 = (uint32_t)((t << 20) | (t >> 12)); }
    { uint32_t t = (a14 ^ d4); b22 = (uint32_t)((t << 7) | (t >> 25)); }
    { uint32_t t = (a19 ^ d4); b13 = (uint32_t)((t << 8) | (t >> 24)); }
    { uint32_t t = (a24 ^ d4); b4 = (uint32_t)((t << 14) | (t >> 18)); }
    a0 = b0 ^ (~b1 & b2);
    a1 = b1 ^ (~b2 & b3);
    a2 = b2 ^ (~b3 & b4);
    a3 = b3 ^ (~b4 & b0);
    a4 = b4 ^ (~b0 & b1);
    a5 = b5 ^ (~b6 & b7);
    a6 = b6 ^ (~b7 & b8);
    a7 = b7 ^ (~b8 & b9);
    a8 = b8 ^ (~b9 & b5);
    a9 = b9 ^ (~b5 & b6);
    a10 = b10 ^ (~b11 & b12);
    a11 = b11 ^ (~b12 & b13);
    a12 = b12 ^ (~b13 & b14);
    a13 = b13 ^ (~b14 & b10);
    a14 = b14 ^ (~b10 & b11);
    a15 = b15 ^ (~b16 & b17);
    a16 = b16 ^ (~b17 & b18);
    a17 = b17 ^ (~b18 & b19);
    a18 = b18 ^ (~b19 & b15);
    a19 = b19 ^ (~b15 & b16);
    a20 = b20 ^ (~b21 & b22);
    a21 = b21 ^ (~b22 & b23);
    a22 = b22 ^ (~b23 & b24);
    a23 = b23 ^ (~b24 & b20);
    a24 = b24 ^ (~b20 & b21);
    a0 ^= (uint32_t)RC[r];
  }
  A[0] = a0;
  A[1] = a1;
  A[2] = a2;
  A[3] = a3;
  A[4] = a4;
  A[5] = a5;
  A[6] = a6;
  A[7] = a7;
  A[8] = a8;
  A[9] = a9;
  A[10] = a10;
  A[11] = a11;
  A[12] = a12;
  A[13] = a13;
  A[14] = a14;
  A[15] = a15;
  A[16] = a16;
  A[17] = a17;
  A[18] = a18;
  A[19] = a19;
  A[20] = a20;
  A[21] = a21;
  A[22] = a22;
  A[23] = a23;
  A[24] = a24;
}
typedef struct { const char *name; int w, rate, first, last; uint8_t dom; } alg_t;
static const alg_t ALGS[] = {
  {"shake128", 64, 168, 0, 23, 0x1F}, {"shake256", 64, 136, 0, 23, 0x1F},
  {"turboshake128", 64, 168, 12, 23, 0x1F}, {"turboshake256", 64, 136, 12, 23, 0x1F},
  {"minishake128", 32, 68, 10, 21, 0x1F},
  {"minishake256", 32, 36, 10, 21, 0x1F},   /* rate 36 is NOT yet checked against the MiniSHAKE paper */
};
static void xof(const alg_t *a, const uint8_t *m, size_t n, uint8_t *out, size_t outlen) {
  union { uint64_t l64[25]; uint32_t l32[25]; uint8_t b[200]; } s; size_t i, pos = 0;
  memset(&s, 0, sizeof s);                                   /* little-endian host assumed (ARM, x86) */
  for (i = 0; i < n; ) {                                      /* absorb a block at a time: the inner loop vectorizes */
    size_t k = (size_t)a->rate - pos, j; if (k > n - i) k = n - i;
    for (j = 0; j < k; j++) s.b[pos + j] ^= m[i + j];
    pos += k; i += k;
    if (pos == (size_t)a->rate) { if (a->w == 64) perm64(s.l64, a->first, a->last); else perm32(s.l32, a->first, a->last); pos = 0; } }
  s.b[pos] ^= a->dom; s.b[a->rate - 1] ^= 0x80;
  if (a->w == 64) perm64(s.l64, a->first, a->last); else perm32(s.l32, a->first, a->last);
  for (i = 0; i < outlen; ) {                                 /* squeeze a block at a time */
    size_t k = (size_t)a->rate; if (k > outlen - i) k = outlen - i;
    memcpy(out + i, s.b, k); i += k;
    if (i < outlen) { if (a->w == 64) perm64(s.l64, a->first, a->last); else perm32(s.l32, a->first, a->last); } }
}
static double now_ns(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec * 1e9 + t.tv_nsec; }
static const alg_t *find_alg(const char *n) { size_t i; for (i = 0; i < sizeof ALGS / sizeof ALGS[0]; i++) if (!strcmp(n, ALGS[i].name)) return &ALGS[i]; return NULL; }
/* application workloads and CPU-side driver cost (no device is modelled here: this measures only what the CPU does) */
static int apps(int argc, char **argv) {
  static uint8_t buf[1 << 16], nodes[4096][32]; static volatile uint64_t reg[64]; double t0, t1; long i, j, n, count; const alg_t *a;
  if (!strcmp(argv[1], "merkle") && argc >= 4) {            /* merkle <alg> <leaves>: 32-byte leaves, 32-byte nodes */
    a = find_alg(argv[2]); n = strtol(argv[3], 0, 10); if (!a || n > 4096 || n < 2) return 2;
    t0 = now_ns();
    for (i = 0; i < n; i++) { memset(buf, (int)i, 32); xof(a, buf, 32, nodes[i], 32); }
    for (j = n; j > 1; j /= 2) for (i = 0; i < j / 2; i++) { memcpy(buf, nodes[2*i], 32); memcpy(buf + 32, nodes[2*i+1], 32); xof(a, buf, 64, nodes[i], 32); }
    t1 = now_ns(); printf("RESULT app=merkle alg=%s n=%ld hashes=%ld ns_total=%.1f check=%u\n", a->name, n, 2*n - 1, t1 - t0, nodes[0][0]); return 0; }
  if (!strcmp(argv[1], "chains") && argc >= 5) {            /* chains <alg> <chains> <steps>: x = H(x), 32 bytes */
    long c = strtol(argv[3], 0, 10), st = strtol(argv[4], 0, 10); a = find_alg(argv[2]); if (!a || c > 4096) return 2;
    t0 = now_ns();
    for (i = 0; i < c; i++) { memset(nodes[i], (int)i, 32); for (j = 0; j < st; j++) xof(a, nodes[i], 32, nodes[i], 32); }
    t1 = now_ns(); printf("RESULT app=chains alg=%s n=%ld hashes=%ld ns_total=%.1f check=%u\n", a->name, c, c * st, t1 - t0, nodes[0][0]); return 0; }
  if (!strcmp(argv[1], "driver") && argc >= 5) {            /* driver <mmio|dma|master> <msglen> <count> */
    n = strtol(argv[3], 0, 10); count = strtol(argv[4], 0, 10); if (n > 4096) return 2;
    for (i = 0; i < n; i++) buf[i] = (uint8_t)i;
    t0 = now_ns();
    for (j = 0; j < count; j++) { buf[0] = (uint8_t)j;
      if (!strcmp(argv[2], "mmio")) { uint64_t w; reg[0] = 1; reg[1] = (uint64_t)n;
        for (i = 0; i < n; i += 8) { memcpy(&w, buf + i, 8); reg[2] = w; } reg[3] = 1; for (i = 0; i < 4; i++) nodes[0][8*i] = (uint8_t)reg[4 + i]; }
      else if (!strcmp(argv[2], "dma")) { memcpy(buf + 8192, buf, (size_t)n); for (i = 0; i < 12; i++) reg[8 + i] = (uint64_t)(j + i); reg[3] = 1; }
      else { memcpy(buf + 8192, buf, (size_t)n); reg[0] = 1; reg[1] = (uint64_t)n; reg[2] = 8192; reg[3] = 12288; } }
    t1 = now_ns(); printf("RESULT app=driver style=%s len=%ld count=%ld ns_per_job=%.1f check=%u\n", argv[2], n, count, (t1 - t0) / count, (unsigned)reg[3]); return 0; }
  return -1;
}
int main(int argc, char **argv) {
  const alg_t *a = NULL; size_t i, n, o; static uint8_t msg[1 << 16], out[1 << 12]; unsigned sum = 0;
  if (argc >= 4) { int r = apps(argc, argv); if (r >= 0) return r; }
  if (argc < 5) { fprintf(stderr, "usage: %s alg msglen outlen count|hex\n", argv[0]); return 2; }
  for (i = 0; i < sizeof ALGS / sizeof ALGS[0]; i++) if (!strcmp(argv[1], ALGS[i].name)) a = &ALGS[i];
  n = strtoul(argv[2], 0, 10); o = strtoul(argv[3], 0, 10);
  if (!a || n > sizeof msg || o > sizeof out) { fprintf(stderr, "bad arguments\n"); return 2; }
  for (i = 0; i < n; i++) msg[i] = (uint8_t)i;
  if (!strcmp(argv[4], "hex")) { xof(a, msg, n, out, o); for (i = 0; i < o; i++) printf("%02x", out[i]); printf("\n"); return 0; }
  { long c, count = strtol(argv[4], 0, 10); double t0, t1;
    for (c = 0; c < 4; c++) { msg[0] = (uint8_t)c; xof(a, msg, n, out, o); sum += out[0]; }        /* warm caches */
    t0 = now_ns();
    for (c = 0; c < count; c++) { msg[0] = (uint8_t)c; xof(a, msg, n, out, o); sum += out[0]; }
    t1 = now_ns();
    printf("RESULT alg=%s len=%zu out=%zu count=%ld ns_per_hash=%.1f check=%u\n", a->name, n, o, count, (t1 - t0) / count, sum); }
  return 0;
}
