# AGENTS.md — Bitcoin Kangaroo Solver

## Goal
Pollard's Kangaroo (lambda-method) Bitcoin puzzle solver in Rust with CPU multi-thread + optional CUDA GPU, checkpoint/resume, Telegram notification.

## Current Status
✅ **Working — all 26 tests pass, 0 warnings, clean compile**

## Architecture
```
src/
├── main.rs                 # CLI: clap args → config → solver dispatch
├── kangaroo/
│   ├── mod.rs
│   ├── point.rs            # secp256k1: Scalar, ProjectivePoint, jump table, address derivation
│   ├── walk.rs             # KangarooWalk: step(), is_distinguished(), affine X cache
│   ├── collision.rs        # CollisionFinder: DistPointEntry + X-based detection
│   ├── params.rs           # KangarooParams: all solver configuration
│   └── distinguished.rs    # is_distinguished() via SHA256 leading-zero bits
├── solver/
│   ├── mod.rs              # Solver trait
│   ├── cpu/mod.rs          # Rayon parallel: tame+wild per thread, shared Arc<Mutex<Vec<DistPointEntry>>>
│   └── gpu/mod.rs          # cust CUDA init + kernel launch + CPU fallback (feature-gated)
├── checkpoint/mod.rs       # bincode serialization of distinguished points + metadata
├── notification/mod.rs     # FoundKey → Telegram (blocking reqwest), console, log file
└── puzzle/mod.rs           # Built-in puzzle #66-#74 table
kernels/
└── kangaroo.cu             # CUDA kernel: Jacobian point arithmetic, jump table, distinguished detection
build.rs                    # nvcc compilation of kangaroo.cu (gpu feature)
tests/
└── integration_test.rs     # 26 tests: arithmetic, walk, collision, checkpoint, address
```

## Key Decisions
| Decision | Rationale |
|----------|-----------|
| Pollard's Kangaroo over brute-force | O(√N) vs O(N) when pubkey is known |
| Distinguished points | Parallel collision detection without central coordination |
| `k256` 0.13.4 | Pure Rust secp256k1, stable API |
| Shared `Arc<Mutex<Vec<DistPointEntry>>>` | Cross-thread tame↔wild collisions (critical: per-thread Vec made cross-thread detection impossible) |
| GPU: nvcc → PTX → cust | No pre-compiled binaries; runtime CUDA detection |
| `build.rs` gated on `gpu` feature | Zero CUDA dependency for pure-CPU builds |
| `rand::Rng` trait for fill | Required for both thread-distinct and regular random generation |

## Critical Bug Fixes
1. **`compute_bitcoin_address`** → was using `to_encoded_point(false)` (uncompressed 04+XY) producing wrong address for Bitcoin Puzzle P2PKH; now uses `to_encoded_point(true)` (02/03+X) matching all puzzle addresses.
2. **Checkpoint resume** → was calling `start_distance_from_checkpoint` which set new kangaroos to old distinguished-point distances. Distances from different runs are incomparable; computing private key as `tame_dist_new - wild_dist_old` would produce garbage. Now starts all kangaroos fresh; old distinguished points serve only as collision database.
3. **`point_to_affine_bytes`** → panicked on point-at-infinity (1-byte `EncodedPoint`). Now returns `[0u8; 33]` (leading 0x00) for zero point.
4. **`k256` 0.13 API** → `Scalar::from_repr` takes `GenericArray`, `ProjectivePoint::from(AffinePoint)` for conversion, `EncodedPoint` for serialization.

## Dependencies (cleaned)
Removed unused: `tokio`, `serde_json`, `indicatif`, `chrono`, `tracing`. All synchronous (blocking reqwest), no async runtime.

## Known Limitations
- ❌ Cannot test on unsolved puzzles #66-#73 (address-only, no public key — Kangaroo requires pubkey)
- ❌ GPU path blocked: no CUDA toolkit on this system (kernel written, build.rs configured, cust init coded)
- ❌ Checkpoint resume starts fresh kangaroos with old distinguished points → old/new points cannot cross-collide; only new↔new collisions are detected after resume

## CLI
```
bitcoin-kangaroo-solver [OPTIONS] --pubkey <HEX>

  --puzzle <N>               Built-in puzzle 66-74
  --start-range <HEX>        Custom start range
  --end-range <HEX>          Custom end range
  --address <ADDR>           Target Bitcoin address
  --pubkey <HEX>             Target public key (compressed, 66 hex)  [required]
  --threads <N>              CPU threads (default: all cores)
  --gpu                      Enable GPU solver
  --checkpoint <PATH>        Checkpoint file path
  --checkpoint-interval <N>  Save interval in seconds (default: 120)
  --log <PATH>               Log file path
  --telegram-bot-token <T>   Telegram bot token
  --telegram-chat-id <ID>    Telegram chat ID
  --list-puzzles             List built-in puzzles
```

## Tests
```
cargo test                           # 26 unit/integration tests (~45s)
cargo test -- --ignored              # GPU tests (requires CUDA)
cargo check                          # 0 warnings
```

## Build
```
cargo build --release                # CPU-only release
cargo build --release --features gpu # CPU+GPU (requires CUDA toolkit)
```
