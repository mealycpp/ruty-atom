// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Mohamed El-Hadedy
// ruty_test: drive the RUTY engine from the ARM through its AXI4-Lite registers and check every digest against software.
// build: gcc -O2 -o ruty_test ruty_test.c      run: sudo ./ruty_test [base hex, default a0000000]
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <fcntl.h>
#include <time.h>
#include <sys/mman.h>
static volatile uint32_t *R;
#define CTRL 0
#define MODE 1
#define ROUNDS0 2
#define ROUNDS1 3
#define STATUS 4
#define CLOCKS0 5
#define CLOCKS1 6
#define ID 7
#define LANE(i, h) (0x100/4 + 2*(i) + (h))
#define XLANE(i, h) (0x200/4 + 2*(i) + (h))
static double now(void){ struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec * 1e9 + t.tv_nsec; }
// ---- software reference: Keccak-p with 64-bit or 32-bit lanes ----
static const int RHO[25] = {0,1,62,28,27,36,44,6,55,20,3,10,43,25,39,41,45,15,21,8,18,2,61,56,14};
static uint64_t RC[24];
static void mkrc(void){ uint8_t l = 1; for (int r = 0; r < 24; r++){ RC[r] = 0; for (int j = 0; j < 7; j++){ if (l & 1) RC[r] |= 1ULL << ((1 << j) - 1); l = (l << 1) ^ ((l & 0x80) ? 0x71 : 0); } } }
static uint64_t rol(uint64_t x, int n, int w){ n %= w; uint64_t m = w == 64 ? ~0ULL : 0xffffffffULL; return n ? ((x << n) | (x >> (w - n))) & m : x; }
static void perm(uint64_t *A, int w, int first, int last){ uint64_t m = w == 64 ? ~0ULL : 0xffffffffULL, C[5], B[25];
  for (int r = first; r <= last; r++){
    for (int x = 0; x < 5; x++) C[x] = A[x] ^ A[x+5] ^ A[x+10] ^ A[x+15] ^ A[x+20];
    for (int i = 0; i < 25; i++) A[i] ^= C[(i%5+4)%5] ^ rol(C[(i%5+1)%5], 1, w);
    for (int x = 0; x < 5; x++) for (int y = 0; y < 5; y++) B[y + 5*((2*x+3*y)%5)] = rol(A[x+5*y], RHO[x+5*y], w);
    for (int i = 0; i < 25; i++) A[i] = B[i] ^ (~B[(i%5+1)%5 + 5*(i/5)] & m & B[(i%5+2)%5 + 5*(i/5)]);
    A[0] ^= RC[r] & m; } }
static void sw_xof(const uint8_t *msg, int len, uint8_t *out, int outlen, int w, int rate, int first, int last){
  int nb = w / 8; uint8_t st[200] = {0}; uint64_t A[25]; int pos = 0, o = 0;
  #define P() { for (int i = 0; i < 25; i++){ A[i] = 0; memcpy(&A[i], st + nb*i, nb); } perm(A, w, first, last); for (int i = 0; i < 25; i++) memcpy(st + nb*i, &A[i], nb); }
  for (int i = 0; i < len; i++){ st[pos++] ^= msg[i]; if (pos == rate){ P(); pos = 0; } }
  st[pos] ^= 0x1F; st[rate-1] ^= 0x80;
  while (o < outlen){ P(); int n = outlen - o < rate ? outlen - o : rate; memcpy(out + o, st, n); o += n; } }
// ---- the same sponge on the engine. half: -1 = whole 64-bit lanes (one core), 0 / 1 = core 0 / core 1 (32-bit lanes) ----
static void go(int mask){ R[CTRL] = mask; while (R[STATUS] & mask) ; }
static void hw_xof(const uint8_t *msg, int len, uint8_t *out, int outlen, int half, int rate){
  int nb = half < 0 ? 8 : 4, mask = half == 1 ? 2 : 1, o = 0; static uint8_t p[8192]; int n = len + 1; n += (rate - n % rate) % rate;
  memset(p, 0, n); memcpy(p, msg, len); p[len] = 0x1F; p[n-1] ^= 0x80;
  for (int i = 0; i < 25; i++){ if (half != 1) R[LANE(i, 0)] = 0; if (half != 0) R[LANE(i, 1)] = 0; }
  for (int b = 0; b < n; b += rate){
    for (int i = 0; i < rate / nb; i++){ uint32_t w[2] = {0, 0}; memcpy(w, p + b + nb*i, nb);
      if (half < 0){ R[XLANE(i, 0)] = w[0]; R[XLANE(i, 1)] = w[1]; } else R[XLANE(i, half)] = w[0]; }
    go(mask); }
  for (;;){
    for (int i = 0; i < rate / nb && o < outlen; i++){ uint32_t w[2]; int k = outlen - o < nb ? outlen - o : nb;
      if (half < 0){ w[0] = R[LANE(i, 0)]; w[1] = R[LANE(i, 1)]; } else w[0] = R[LANE(i, half)];
      memcpy(out + o, w, k); o += k; }
    if (o >= outlen) break; go(mask); } }
int main(int argc, char **argv){
  unsigned long base = argc > 1 ? strtoul(argv[1], 0, 16) : 0xa0000000UL; int fd = open("/dev/mem", O_RDWR | O_SYNC);
  if (fd < 0){ perror("open /dev/mem (run with sudo)"); return 1; }
  R = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_SHARED, fd, base); if (R == MAP_FAILED){ perror("mmap"); return 1; }
  mkrc(); uint32_t id = R[ID]; printf("RESULT id=%08x slices_per_clock=%u version=%u\n", id, (id >> 8) & 255, id & 255);
  if ((id >> 16) != 0x5255){ printf("RESULT error=engine_not_found\n"); return 1; }
  static uint8_t msg[4096], a[512], b[512]; srand(7); for (int i = 0; i < 4096; i++) msg[i] = rand();
  struct { const char *name; int w, rate, first, last; } alg[] = { {"shake128", 64, 168, 0, 23}, {"shake256", 64, 136, 0, 23},
    {"turboshake128", 64, 168, 12, 23}, {"turboshake256", 64, 136, 12, 23}, {"minishake128", 32, 68, 10, 21}, {"minishake256", 32, 36, 10, 21} };
  int lens[] = {0, 1, 32, 67, 68, 135, 136, 167, 168, 169, 1000, 4096}, bad = 0, total = 0;
  for (int k = 0; k < 6; k++){ int fail = 0;
    R[MODE] = alg[k].w == 32; R[ROUNDS0] = alg[k].first | (alg[k].last << 8); R[ROUNDS1] = R[ROUNDS0];
    for (int j = 0; j < 12; j++) for (int half = (alg[k].w == 32 ? 0 : -1); half <= (alg[k].w == 32 ? 1 : -1); half++){
      sw_xof(msg, lens[j], a, 300, alg[k].w, alg[k].rate, alg[k].first, alg[k].last);
      hw_xof(msg, lens[j], b, 300, half, alg[k].rate); fail += memcmp(a, b, 300) != 0; total++; }
    bad += fail; printf("RESULT check alg=%s lengths=12 out=300 mismatches=%d\n", alg[k].name, fail); }
  // engine time counted in hardware, and end-to-end time of one short hash seen by the program
  R[MODE] = 0; R[ROUNDS0] = 0 | (23 << 8); go(1); uint32_t c24 = R[CLOCKS0];
  R[ROUNDS0] = 12 | (23 << 8); go(1); uint32_t c12 = R[CLOCKS0];
  R[MODE] = 1; R[ROUNDS0] = 10 | (21 << 8); R[ROUNDS1] = 10 | (21 << 8); go(3); uint32_t d0 = R[CLOCKS0], d1 = R[CLOCKS1];
  printf("RESULT engine_clocks rounds24=%u rounds12=%u two_cores_12=%u,%u\n", c24, c12, d0, d1);
  for (int k = 0; k < 6; k += 2){ int n = 20000; R[MODE] = alg[k].w == 32; R[ROUNDS0] = alg[k].first | (alg[k].last << 8);
    double t0 = now(); for (int i = 0; i < n; i++) hw_xof(msg, 32, b, 32, alg[k].w == 32 ? 0 : -1, alg[k].rate); double t1 = now();
    for (int i = 0; i < n; i++) sw_xof(msg, 32, a, 32, alg[k].w, alg[k].rate, alg[k].first, alg[k].last); double t2 = now();
    printf("RESULT time alg=%s len=32 out=32 through_registers_ns=%.0f plain_c_reference_ns=%.0f\n", alg[k].name, (t1 - t0) / n, (t2 - t1) / n); }
  printf("RESULT summary checks=%d mismatches=%d %s\n", total, bad, bad ? "FAIL" : "ALL_MATCH"); return bad != 0; }
