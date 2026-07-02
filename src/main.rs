use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use bitcoin_kangaroo_solver::{
    checkpoint::Checkpoint,
    kangaroo::KangarooParams,
    notification::{Notifier, Notify},
    puzzle,
    solver::{cpu::CpuSolver, Solver},
    APP_NAME, APP_VERSION, INTERRUPTED,
};

#[derive(Parser, Debug)]
#[command(name = APP_NAME)]
#[command(version = APP_VERSION)]
#[command(about = "Pollard's Kangaroo algorithm based Bitcoin puzzle solver")]
struct Args {
    #[arg(short, long, help = "Puzzle number (135, 140, 145, 150, 155, 160). Uses built-in range, address & pubkey.")]
    puzzle: Option<u32>,

    #[arg(long, help = "Start range in hex (32 bytes). Required if --puzzle not set.")]
    start_range: Option<String>,

    #[arg(long, help = "End range in hex (32 bytes). Required if --puzzle not set.")]
    end_range: Option<String>,

    #[arg(long, help = "Target Bitcoin address (for display). Required if --puzzle not set.")]
    address: Option<String>,

    #[arg(long, help = "Target compressed public key in hex (66 hex chars, 02/03 prefix). REQUIRED if --puzzle not set or puzzle has no embedded pubkey.")]
    pubkey: Option<String>,

    #[arg(short, long, help = "Number of CPU threads (default: half of available cores)")]
    threads: Option<usize>,

    #[arg(short = 'g', long, help = "GPU backend: 'cuda' or 'wgpu'")]
    gpu: Option<String>,

    #[arg(short = 'c', long, help = "Path to checkpoint file to resume from")]
    checkpoint: Option<PathBuf>,

    #[arg(long, help = "Telegram bot token")]
    telegram_token: Option<String>,

    #[arg(long, help = "Telegram chat ID")]
    telegram_chat_id: Option<String>,

    #[arg(long, default_value = "300", help = "Checkpoint save interval in seconds")]
    checkpoint_interval: u64,

    #[arg(long, default_value = "20", help = "Distinguished point bit count")]
    distinguished_bits: Option<u32>,

    #[arg(long, help = "Log file path for found keys")]
    log_file: Option<PathBuf>,

    #[arg(long, default_value_t = true, help = "Enable negation map (2x search speed)")]
    negation_map: bool,

    #[arg(long, default_value_t = true, help = "Use SOTA K=1.15 jump table")]
    sota: bool,

    #[arg(long, short = 'l', action = clap::ArgAction::SetTrue, help = "List available puzzles")]
    list: bool,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env(),
        )
        .init();

    let args = Args::parse();

    if args.list {
        println!("\n{} v{} - Available Puzzles", APP_NAME, APP_VERSION);
        println!("{}", "=".repeat(60));
        for p in puzzle::get_active_puzzles() {
            let kangaroo_ok = p.pubkey.is_some();
            let status = if kangaroo_ok { "PUBKEY ACIK" } else { "pubkey yok" };
            let marker = if kangaroo_ok { " [Kangaroo]"} else { "" };
            println!("  #{:<4} 2^{:<4} -> 2^{:<4}  {}  {}{}", p.id, p.id - 1, p.id, p.address, status, marker);
        }
        println!();
        return;
    }

    let (puzzle_id, start_range, end_range, target_address, target_pubkey) =
        if let Some(pnum) = args.puzzle {
            let puzzles = puzzle::get_active_puzzles();
            let p = puzzles.iter().find(|p| p.id == pnum)
                .unwrap_or_else(|| {
                    eprintln!("[ERROR] Puzzle #{} not found. Use --list to see available puzzles.", pnum);
                    std::process::exit(1);
                });

            let pubkey = if let Some(ref pk) = p.pubkey {
                hex_to_pubkey(pk)
            } else if let Some(ref pk_arg) = args.pubkey {
                hex_to_pubkey(pk_arg)
            } else {
                eprintln!("[ERROR] Puzzle #{} has no embedded pubkey. Provide --pubkey manually.", pnum);
                std::process::exit(1);
            };

            (p.id, range_to_bytes(p.power - 1), range_to_bytes(p.power), p.address.clone(), pubkey)
        } else {
            let start = hex_to_32(&args.start_range.as_ref()
                .unwrap_or_else(|| { eprintln!("[ERROR] --start-range required when --puzzle not set"); std::process::exit(1); }));
            let end = hex_to_32(&args.end_range.as_ref()
                .unwrap_or_else(|| { eprintln!("[ERROR] --end-range required when --puzzle not set"); std::process::exit(1); }));
            let addr = args.address.as_ref()
                .unwrap_or_else(|| { eprintln!("[ERROR] --address required when --puzzle not set"); std::process::exit(1); });
            let pubkey = args.pubkey.as_ref()
                .map(|pk| hex_to_pubkey(pk))
                .unwrap_or_else(|| { eprintln!("[ERROR] --pubkey required when --puzzle not set or no embedded pubkey"); std::process::exit(1); });
            (0, start, end, addr.clone(), pubkey)
        };

    let num_threads = args.threads.unwrap_or_else(|| {
        let cores = num_cpus::get();
        (cores / 2).max(1)
    });

    let distinguished_bits = args.distinguished_bits.unwrap_or(20);
    let checkpoint_path = args.checkpoint.as_ref().map(|p| p.to_string_lossy().to_string());

    let params = KangarooParams::new(
        puzzle_id,
        start_range,
        end_range,
        target_pubkey,
        num_threads,
        distinguished_bits,
        checkpoint_path.clone(),
        args.checkpoint_interval,
    )
    .with_negation_map(args.negation_map)
    .with_sota_mode(args.sota);

    let maybe_checkpoint = checkpoint_path.as_ref().and_then(|path| {
        let pb = PathBuf::from(path);
        if pb.exists() {
            match Checkpoint::load(&pb) {
                Ok(ckpt) => {
                    println!("[INFO] Loaded checkpoint from {} ({} ops, {} dist points)",
                        path, ckpt.total_ops, ckpt.distinguished_points.len());
                    Some(ckpt)
                }
                Err(e) => {
                    eprintln!("[WARN] Failed to load checkpoint: {}. Starting fresh.", e);
                    None
                }
            }
        } else {
            None
        }
    });

    let notifier = Notifier::new(
        args.telegram_token,
        args.telegram_chat_id,
        args.log_file,
    );

    ctrlc::set_handler(move || {
        INTERRUPTED.store(true, Ordering::SeqCst);
        eprintln!("\n[!] Interrupt signal received. Saving checkpoint and shutting down...");
    }).expect("Error setting Ctrl-C handler");

    println!();
    println!("{} v{}", APP_NAME, APP_VERSION);
    println!("{}", "=".repeat(60));
    println!("  Puzzle       : #{}", puzzle_id);
    println!("  Target       : {}", target_address);
    println!("  Algorithm    : Pollard's Kangaroo (Distinguished Points)");
    println!("  Threads      : {} (CPU)", params.num_threads);
    if args.gpu.is_some() {
        println!("  GPU          : ENABLED ({})", args.gpu.as_deref().unwrap_or("cuda").to_uppercase());
    }
    println!("  Negation Map : {}", if params.negation_map { "ON" } else { "OFF" });
    println!("  SOTA K=1.15  : {}", if params.sota_mode { "ON" } else { "OFF" });
    println!("  Distinguished: {} bits", params.distinguished_bit);
    println!("  Checkpoint   : {}", match &params.checkpoint_path {
        Some(p) => format!("{} (every {}s)", p, params.checkpoint_interval),
        None => "disabled".into(),
    });
    println!("{}", "=".repeat(60));

    let start = Instant::now();

    if let Some(ref gpu_backend) = args.gpu {
        match gpu_backend.as_str() {
            "cuda" => {
                #[cfg(feature = "gpu")]
                {
                    let solver = bitcoin_kangaroo_solver::solver::gpu::GpuSolver;
                    solver.run(&params, maybe_checkpoint.as_ref(), &notifier as &dyn Notify);
                }
                #[cfg(not(feature = "gpu"))]
                {
                    eprintln!("[ERROR] CUDA GPU feature not enabled. Rebuild with: cargo build --release --features=gpu");
                    std::process::exit(1);
                }
            }
            "wgpu" => {
                #[cfg(feature = "gpu-wgpu")]
                {
                    let solver = bitcoin_kangaroo_solver::solver::wgpu_solver::WgpuSolver;
                    solver.run(&params, maybe_checkpoint.as_ref(), &notifier as &dyn Notify);
                }
                #[cfg(not(feature = "gpu-wgpu"))]
                {
                    eprintln!("[ERROR] wgpu GPU feature not enabled. Rebuild with: cargo build --release --features=gpu-wgpu");
                    std::process::exit(1);
                }
            }
            other => {
                eprintln!("[ERROR] Unknown GPU backend '{}'. Use 'cuda' or 'wgpu'.", other);
                std::process::exit(1);
            }
        }
    } else {
        let solver = CpuSolver;
        solver.run(&params, maybe_checkpoint.as_ref(), &notifier as &dyn Notify);
    }

    let elapsed = start.elapsed();
    println!("\nTotal runtime: {}s", elapsed.as_secs());

}

fn hex_to_32(hex: &str) -> [u8; 32] {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    let bytes = hex::decode(hex).expect("Invalid hex (expected 32 bytes / 64 hex chars)");
    assert_eq!(bytes.len(), 32, "Hex must be 64 characters (32 bytes)");
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

fn hex_to_pubkey(hex: &str) -> [u8; 33] {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    let bytes = hex::decode(hex).expect("Invalid hex in --pubkey");
    assert_eq!(bytes.len(), 33, "Public key must be 33 bytes (66 hex chars)");
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&bytes);
    arr
}

fn range_to_bytes(power: u32) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    if power == 0 {
        bytes[31] = 1;
        return bytes;
    }
    let idx = 31 - (power as usize / 8);
    let bit = power % 8;
    bytes[idx] = 1 << bit;
    bytes
}
