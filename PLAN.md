# 🎯 Strategic Roadmap — Bitcoin Kangaroo Solver

> **Goal:** Become the undisputed #1 Bitcoin puzzle solver — faster, safer, and more portable than all alternatives combined.
> **Method:** Phased execution, each phase ends with commit+push.
> **Last updated:** 2026-07-01

---

## Phase 0 — GPU Kernel Rewrite (Montgomery + Workgroup)
**Duration:** ~2 weeks
**Impact:** 10-50x GPU speedup → matches or exceeds JLP Kangaroo perf

### Tasks
- [ ] **0.1** Replace schoolbook 32-bit limb multiplication with **Montgomery multiplication** (CIOS method, 64-bit intermediates)
- [ ] **0.2** Change `@workgroup_size(1)` → `@workgroup_size(64)` with shared memory for jump table
- [ ] **0.3** Replace Fermat modular inverse with **Montgomery inverse** or precomputation strategy
- [ ] **0.4** Minimize register pressure — reduce array copies, use `var` reuse
- [ ] **0.5** Verify kernel correctness: E2E test with random keys
- [ ] **0.6** Benchmark vs baseline: measure MKey/s on RX550 and RTX 4090

---

## Phase 1 — Multi-GPU Support
**Duration:** ~1 week
**Impact:** Single process drives N GPUs, matches JLP feature

### Tasks
- [ ] **1.1** Auto-detect all available GPU adapters via wgpu
- [ ] **1.2** Spawn N compute shader instances, one per GPU
- [ ] **1.3** Shared collision database across GPUs (host-side merge)
- [ ] **1.4** CLI flag: `--gpu all` or `--gpu 0,1,2`
- [ ] **1.5** Graceful handling of GPU failure (remove from pool)

---

## Phase 2 — Client/Server Distributed Mode
**Duration:** ~2 weeks
**Impact:** Pool machines across LAN/WAN → unlimited scale

### Tasks
- [ ] **2.1** Design protocol: TCP + bincode, minimal overhead
- [ ] **2.2** Server: range manager, DP collector, collision checker
- [ ] **2.3** Client: connects, receives work units, sends DPs back
- [ ] **2.4** TLS support for WAN connections
- [ ] **2.5** Auto-reconnect, stale client timeout
- [ ] **2.6** CLI: `--server` and `--connect <host>:<port>`

---

## Phase 3 — GPU Kernel Deep Optimization
**Duration:** ~2 weeks
**Impact:** Extract maximum FLOPS from each GPU architecture

### Tasks
- [ ] **3.1** Workgroup size auto-tuning per GPU (NVIDIA warp=32, AMD wavefront=64)
- [ ] **3.2** Memory coalescing: AoS → SoA layout for kangaroo state
- [ ] **3.3** Precompute jump table on GPU (avoid transfer bottleneck)
- [ ] **3.4** Batch DP upload: reduce PCIe round-trips
- [ ] **3.5** Double-ladder collision check directly in kernel (no host round-trip for common case)
- [ ] **3.6** Profile with NSight/Radeon GPU Profiler, identify remaining bottlenecks

---

## Phase 4 — BSGS Mode (Deterministic Solver)
**Duration:** ~3 weeks
**Impact:** Two algorithms in one tool — Kangaroo (probabilistic) + BSGS (deterministic)

### Tasks
- [ ] **4.1** Implement secp256k1 BSGS core with bloom filters
- [ ] **4.2** GPU BSGS kernel (parallel baby-step generation)
- [ ] **4.3** Automatic mode selection: BSGS for range ≤ 2^52, Kangaroo for larger
- [ ] **4.4** CLI: `--mode kangaroo|bsgs|auto`
- [ ] **4.5** BSGS checkpoint/resume support

---

## Phase 5 — Address-Only Mode
**Duration:** ~1 week
**Impact:** Solve puzzles #66-#73 without public key (only address known)

### Tasks
- [ ] **5.1** RMD160 hash computation in GPU kernel
- [ ] **5.2** Address → HASH160 → public key generation pipeline
- [ ] **5.3** Benchmark: Maddress/s vs Mkey/s tradeoff
- [ ] **5.4** CLI: `--address <ADDR>` without `--pubkey`

---

## Phase 6 — Endomorphism Optimization
**Duration:** ~1 week
**Impact:** Combined with negation map → 6x search space reduction

### Tasks
- [ ] **6.1** Implement endomorphism map (multiplication by β, λ) in WGSL
- [ ] **6.2** Three simultaneous walks per kangaroo (original, β*P, β²*P)
- [ ] **6.3** Collision detection across all three walks
- [ ] **6.4** Verify correctness with E2E tests

---

## Phase 7 — Polish & Battle-Test
**Duration:** Ongoing
**Impact:** Real-world trust, solved puzzles, community adoption

### Tasks
- [ ] **7.1** Solve at least one real puzzle (#66, #125, etc.)
- [ ] **7.2** Comprehensive benchmarking suite (auto-run on startup)
- [ ] **7.3** Docker image for easy deployment
- [ ] **7.4** GitHub Actions CI: build + test on Windows + Linux
- [ ] **7.5** Performance dashboard: track MKey/s per GPU model
- [ ] **7.6** User documentation: troubleshooting per GPU vendor

---

## Legend

| Priority | Label |
|----------|-------|
| 🔴 Must-have | Performance or feature gap blocking superiority |
| 🟠 Important | Significant feature gap vs competitors |
| 🟡 Nice-to-have | Adds versatility |
| 🔵 Polish | Quality-of-life, trust, community |

## Progress

```
Phase 0 [    ] Phase 1 [    ] Phase 2 [    ] Phase 3 [    ]
Phase 4 [    ] Phase 5 [    ] Phase 6 [    ] Phase 7 [    ]
```
