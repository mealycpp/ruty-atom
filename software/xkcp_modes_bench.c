// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
/* xkcp_modes_bench.c (NEW FILE): MiniSHAKE128 / TurboSHAKE128 / SHAKE128 Merkle trees on the Keccak team's own permutations from XKCP,
   a software comparator that is not our code. Permutations: XKCP commit fa7033a212c831249b7e17e785ee1a4e24fc04c4 (the last commit before
   239d0ac "Remove Keccak-f[200], [400] and [800]"): lib/low/KeccakP-800/plain/KeccakP-800-opt32.c (config ua: all rounds unrolled, flavor bis)
   and lib/low/KeccakP-1600/plain-64bits/KeccakP-1600-opt64.c (config ua: all rounds unrolled, no lane complementing).
   Only the sponge wrapper below is ours: absorb with XKCP AddBytes, pad10*1 with the domain byte (AddByte), XKCP Permute_Nrounds (12 or 24),
   squeeze with XKCP ExtractBytes. MiniSHAKE128: Keccak-p[800, 12] (rounds 10..21 = the last 12 of 22), rate 68 bytes, D = 0x01.
     xkcp_modes_bench digests                      digests of fixed inputs (checked against our two MiniSHAKE implementations BEFORE timing)
     xkcp_modes_bench merkle <reps> <threads> <k,k>  same trees as modes_bench / ruty_modes                                               */
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include "KeccakP-800-SnP.h"
#include "KeccakP-1600-SnP.h"
typedef struct { int w, rate, nr; uint8_t dom; const char *name; } alg_t;
static const alg_t MINI128 = {32, 68, 12, 0x01, "minishake128"}, TURBO128 = {64, 168, 12, 0x1F, "turboshake128"}, SHAKE128 = {64, 168, 24, 0x1F, "shake128"};
static void xkcp_xof(const alg_t *a, uint8_t dom, const uint8_t *m, size_t n, uint8_t *out, size_t olen) {
  uint64_t st[25]; size_t i = 0;
  if (a->w == 32) { KeccakP800_Initialize(st);
    while (i + (size_t)a->rate <= n) { KeccakP800_AddBytes(st, m + i, 0, a->rate); KeccakP800_Permute_Nrounds(st, a->nr); i += a->rate; }
    KeccakP800_AddBytes(st, m + i, 0, (unsigned)(n - i)); KeccakP800_AddByte(st, dom, (unsigned)(n - i)); KeccakP800_AddByte(st, 0x80, a->rate - 1);
    for (i = 0; ; ) { size_t k; KeccakP800_Permute_Nrounds(st, a->nr); k = olen - i < (size_t)a->rate ? olen - i : (size_t)a->rate; KeccakP800_ExtractBytes(st, out + i, 0, (unsigned)k); i += k; if (i >= olen) break; } }
  else { KeccakP1600_Initialize(st);
    while (i + (size_t)a->rate <= n) { KeccakP1600_AddBytes(st, m + i, 0, a->rate); KeccakP1600_Permute_Nrounds(st, a->nr); i += a->rate; }
    KeccakP1600_AddBytes(st, m + i, 0, (unsigned)(n - i)); KeccakP1600_AddByte(st, dom, (unsigned)(n - i)); KeccakP1600_AddByte(st, 0x80, a->rate - 1);
    for (i = 0; ; ) { size_t k; KeccakP1600_Permute_Nrounds(st, a->nr); k = olen - i < (size_t)a->rate ? olen - i : (size_t)a->rate; KeccakP1600_ExtractBytes(st, out + i, 0, (unsigned)k); i += k; if (i >= olen) break; } } }
static double now_ns(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec * 1e9 + t.tv_nsec; }
static int cmpd(const void *x, const void *y) { double a = *(const double *)x, b = *(const double *)y; return a < b ? -1 : a > b; }
static void pstats(char *o, double *v, int n) { qsort(v, n, sizeof *v, cmpd);
  sprintf(o, "n=%d min_us=%.2f q1_us=%.2f median_us=%.2f q3_us=%.2f max_us=%.2f spread_max_over_min=%.4f", n, v[0] / 1e3, v[n / 4] / 1e3, v[n / 2] / 1e3, v[3 * n / 4] / 1e3, v[n - 1] / 1e3, v[n - 1] / v[0]); }
static void hex(char *o, const uint8_t *b, int n) { int i; for (i = 0; i < n; i++) sprintf(o + 2 * i, "%02x", b[i]); }
static int NT = 1; static atomic_int gen, done; static void (*job_fn)(int, void *); static void *job_arg;
static void pin(int c) { cpu_set_t cs; CPU_ZERO(&cs); CPU_SET(c, &cs); pthread_setaffinity_np(pthread_self(), sizeof cs, &cs); }
static void *worker(void *p) { int tid = (int)(intptr_t)p, seen = 0; pin(tid);
  for (;;) { while (atomic_load_explicit(&gen, memory_order_acquire) == seen) ; seen = atomic_load(&gen); if (!job_fn) return 0; job_fn(tid, job_arg); atomic_fetch_add(&done, 1); } }
static void pool(int t) { int i; pthread_t th; NT = t; pin(0); for (i = 1; i < t; i++) pthread_create(&th, 0, worker, (void *)(intptr_t)i); }
static void par(void (*fn)(int, void *), void *arg) { if (NT == 1) { fn(0, arg); return; }
  job_fn = fn; job_arg = arg; atomic_store(&done, 0); atomic_fetch_add_explicit(&gen, 1, memory_order_release); fn(0, arg); while (atomic_load(&done) < NT - 1) ; }
static uint8_t rng_byte(uint64_t *x) { *x ^= *x << 13; *x ^= *x >> 7; *x ^= *x << 17; return (uint8_t)(*x >> 24); }
typedef struct { const alg_t *a; const uint8_t *vals; uint8_t *w; size_t n; } mk_t;
static void mk_sub(int tid, void *p) { mk_t *k = p; size_t per = k->n / NT, base = per * tid, i, m; uint8_t *w = k->w + 32 * base;
  for (i = 0; i < per; i++) xkcp_xof(k->a, k->a->dom, k->vals + 32 * (base + i), 32, w + 32 * i, 32);
  for (m = per; m > 1; m /= 2) for (i = 0; i < m / 2; i++) xkcp_xof(k->a, k->a->dom, w + 64 * i, 64, w + 32 * i, 32); }
static void merkle(mk_t *k, uint8_t *root) { size_t m, i; uint8_t top[64 * 8];
  par(mk_sub, k); for (i = 0; i < (size_t)NT; i++) memcpy(top + 32 * i, k->w + 32 * (k->n / NT) * i, 32);
  for (m = NT; m > 1; m /= 2) for (i = 0; i < m / 2; i++) xkcp_xof(k->a, k->a->dom, top + 64 * i, 64, top + 32 * i, 32); memcpy(root, top, 32); }
int main(int argc, char **argv) {
  if (argc >= 2 && !strcmp(argv[1], "digests")) { uint8_t m[1000], out[64]; char h[130]; int i, li; int L[] = {0, 1, 32, 63, 64, 67, 68, 69, 100, 136, 1000};
    for (i = 0; i < 1000; i++) m[i] = (uint8_t)i;
    printf("RESULT xkcp_impl p800=\"%s\" p1600=\"%s\"\n", KeccakP800_implementation, KeccakP1600_implementation);
    /* same input as ruty_apps.rs refvec: MiniSHAKE128, D=0x01, bytes 00..63 (100 bytes), 16-byte output */
    xkcp_xof(&MINI128, 0x01, m, 100, out, 16); hex(h, out, 16); printf("RESULT xdig refvec minishake128 dom=01 msg=00..63 out16=%s\n", h);
    /* same inputs as keccak_bench hex mode (its MiniSHAKE uses D=0x1F): bytes 0,1,2,..., outlen 32 */
    for (li = 0; li < 11; li++) { xkcp_xof(&MINI128, 0x1F, m, L[li], out, 32); hex(h, out, 32); printf("RESULT xdig kbhex minishake128 dom=1f len=%d out32=%s\n", L[li], h); }
    for (li = 0; li < 11; li++) { xkcp_xof(&TURBO128, 0x1F, m, L[li], out, 32); hex(h, out, 32); printf("RESULT xdig kbhex turboshake128 dom=1f len=%d out32=%s\n", L[li], h); }
    for (li = 0; li < 11; li++) { xkcp_xof(&SHAKE128, 0x1F, m, L[li], out, 32); hex(h, out, 32); printf("RESULT xdig kbhex shake128 dom=1f len=%d out32=%s\n", L[li], h); }
    return 0; }
  if (argc >= 5 && !strcmp(argv[1], "merkle")) { int reps = atoi(argv[2]); const char *p = argv[4]; double *t = malloc(sizeof(double) * (reps + 2)); pool(atoi(argv[3]));
    while (*p) { int k = atoi(p), mi, r; size_t n = (size_t)1 << k, i; uint64_t x = 0x4D45524B4C450000ULL + (uint64_t)k; uint8_t *vals = malloc(32 * n), *w = malloc(32 * n), root[32]; char st[300], h[70];
      for (i = 0; i < 32 * n; i++) vals[i] = rng_byte(&x);
      for (mi = 0; mi < 3; mi++) { const alg_t *A[3] = {&MINI128, &TURBO128, &SHAKE128}; mk_t mk = {A[mi], vals, w, n};
        for (r = 0; r < reps + 2; r++) { double t0 = now_ns(); merkle(&mk, root); t[r] = now_ns() - t0; }
        pstats(st, t + 2, reps); hex(h, root, 32); qsort(t + 2, reps, sizeof *t, cmpd);
        printf("RESULT xkcpmerkle k=%d alg=%s threads=%d hashes=%zu %s Mhash_per_s=%.4f root=%s\n", k, A[mi]->name, NT, 2 * n - 1, st, (2 * n - 1) / t[2 + reps / 2] * 1e3, h); }
      free(vals); free(w); while (*p && *p != ',') p++; if (*p) p++; }
    return 0; }
  fprintf(stderr, "usage: xkcp_modes_bench digests | merkle <reps> <threads> <k,k>\n"); return 2; }
