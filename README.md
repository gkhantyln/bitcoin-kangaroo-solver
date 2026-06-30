# Bitcoin Kangaroo Solver

Pollard's Kangaroo (lambda-method) Bitcoin puzzle solver in Rust with CPU multi-threading and optional CUDA GPU acceleration.

## Features

- **Pollard's Kangaroo algorithm** — O(√N) vs O(N) brute-force, requires public key
- **CPU multi-threading** — Rayon parallel tame+wild kangaroo pairs per thread
- **CUDA GPU solver** — Jacobian arithmetic kernel, feature-gated (`--features gpu`)
- **Checkpoint/resume** — Periodic bincode serialization of distinguished points to disk
- **Graceful shutdown** — SIGINT (Ctrl+C) saves checkpoint and exits cleanly
- **Telegram notification** — Blocking reqwest sends FoundKey alert to configured chat
- **Log file** — Appends found keys to file with timestamp
- **Built-in puzzle table** — Puzzles #66-#74 with known ranges and addresses

## Prerequisites

- Rust 1.75+ (edition 2021)
- CUDA toolkit 11+ (only for GPU build)

## Build

```bash
# CPU-only (works everywhere)
cargo build --release

# CPU + GPU (requires CUDA toolkit)
cargo build --release --features gpu
```

Release binary: `target/release/bitcoin-kangaroo-solver.exe`

## Usage

```bash
# List built-in puzzles
bitcoin-kangaroo-solver --list

# Solve puzzle #66 with 8 CPU threads
bitcoin-kangaroo-solver --puzzle 66 --threads 8 --pubkey <COMPRESSED_PUBKEY_HEX>

# Custom range with Telegram notification + checkpoint
bitcoin-kangaroo-solver \
  --start-range 0000000000000000000000000000000000000000000000000000000000000001 \
  --end-range   0000000000000000000000000000000000000000000000000000000001000000 \
  --pubkey 02<64_HEX_CHARS> \
  --address 1PVoXoTNaGWtnFfGAhf1RMycFUssCPnCGE \
  --checkpoint puzzle.cp \
  --telegram-bot-token <BOT_TOKEN> \
  --telegram-chat-id <CHAT_ID>

# Resume from checkpoint
bitcoin-kangaroo-solver --puzzle 66 --checkpoint puzzle.cp --pubkey <HEX>

# GPU solver (requires CUDA)
bitcoin-kangaroo-solver --puzzle 66 --gpu --pubkey <HEX>
```

### Options

| Flag | Description |
|------|-------------|
| `--puzzle <N>` | Built-in puzzle 66-74 (sets range + address) |
| `--start-range <HEX>` | Custom start range (32 bytes hex) |
| `--end-range <HEX>` | Custom end range (32 bytes hex) |
| `--address <ADDR>` | Target Bitcoin address (display only) |
| `--pubkey <HEX>` | Target compressed public key (66 hex, 02/03 prefix) — **REQUIRED** |
| `-t, --threads <N>` | CPU thread count (default: half of available cores) |
| `-g, --gpu` | Enable CUDA GPU solver |
| `-c, --checkpoint <PATH>` | Checkpoint file for resume |
| `--checkpoint-interval <N>` | Checkpoint save interval in seconds (default: 300) |
| `--distinguished-bits <N>` | Distinguished point bit count (default: 20) |
| `--telegram-bot-token <T>` | Telegram bot token |
| `--telegram-chat-id <ID>` | Telegram chat ID |
| `--log <PATH>` | Log file path for found keys |
| `-l, --list` | List built-in puzzles |

## Architecture

```
src/
├── main.rs                 # CLI: clap args → config → solver dispatch
├── lib.rs                  # Module exports, INTERRUPTED flag
├── kangaroo/
│   ├── point.rs            # secp256k1: Scalar, ProjectivePoint, jump table, address derivation
│   ├── walk.rs             # KangarooWalk: step(), is_distinguished(), affine X cache
│   ├── collision.rs        # CollisionFinder: X-coordinate based tame↔wild detection
│   ├── params.rs           # KangarooParams: all solver configuration
│   └── distinguished.rs    # is_distinguished() via SHA256 leading-zero bits
├── solver/
│   ├── mod.rs              # Solver trait
│   ├── cpu/mod.rs          # Rayon parallel solver, shared collision database
│   └── gpu/mod.rs          # CUDA init + CPU fallback (feature-gated)
├── checkpoint/mod.rs       # bincode serialization
├── notification/mod.rs     # FoundKey → Telegram, console, log file
└── puzzle/mod.rs           # Built-in puzzle #66-#74 table
kernels/
└── kangaroo.cu             # CUDA kernel: Jacobian arithmetic, jump table, distinguished detection
```

### How It Works

1. **Two herds of kangaroos** — tame (red) and wild (blue) walk pseudo-randomly through the key space
2. **Distinguished points** — when a kangaroo lands on a point with specific bit pattern, it records its position
3. **Collision** — when tame and wild kangaroos visit the same X-coordinate, the private key is recovered: `privkey = tame_dist - wild_dist`
4. **Parallelism** — each thread runs independent tame+wild pairs; distinguished points shared via `Arc<Mutex<Vec<...>>>`

## Tests

```bash
# All unit + integration tests (~40s)
cargo test

# E2E solver test (generates random key in [1, 10000), verifies solver finds it)
cargo test -- --ignored

# Clean check
cargo check
```

## Limitations

- **Requires public key** — Kangaroo algorithm cannot work with Bitcoin address alone. Puzzles #66-#73 are address-only publicly; you must obtain the compressed public key from out-of-band sources
- **Checkpoint resume trade-off** — old distinguished points from previous run are kept as collision database, but all kangaroos start fresh (stale distances are incomparable across runs)
- **GPU untested** — CUDA path is written and compiles but awaits hardware to validate

## License

MIT
