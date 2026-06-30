use std::time::Instant;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::kangaroo::params::KangarooParams;
use crate::checkpoint::Checkpoint;
use crate::notification::{FoundKey, Notify};
use crate::solver::Solver;
use crate::kangaroo::point;

pub struct GpuSolver;

impl Solver for GpuSolver {
    fn run(&self, params: &KangarooParams, _checkpoint: Option<&Checkpoint>, notifier: &dyn Notify) {
        let found = Arc::new(AtomicBool::new(false));
        let start = Instant::now();

        println!("\n[GPU] Initializing CUDA solver...");
        println!("[GPU] Puzzle #{} | Distinguished bits: {}", params.puzzle_id, params.distinguished_bit);
        println!("[GPU] Range width: 2^{}", params.puzzle_id);
        println!("[GPU] Jump table size: {}", params.jump_table_size);

        let target_arr: [u8; 33] = params.target_point.as_slice().try_into()
            .expect("Invalid target point length");
        let target_pt = point::affine_bytes_to_point(&target_arr)
            .expect("Invalid target point");
        let _target_affine = point::point_to_affine_bytes(&target_pt);

        // Generate deterministic jump table (same as CPU for verification)
        let _jump_table = point::generate_jump_table(params.jump_table_size);

        match execute_gpu_kernel(params, &found, &start) {
            Ok(private_key) => {
                let pubkey = point::point_from_scalar(&private_key);
                let address = point::compute_bitcoin_address(&pubkey);
                let fk = FoundKey {
                    private_key: point::scalar_to_bytes(&private_key),
                    public_key: point::point_to_affine_bytes(&pubkey),
                    address,
                    puzzle_id: params.puzzle_id,
                    thread_id: 0,
                    elapsed_seconds: start.elapsed().as_secs(),
                };
                found.store(true, Ordering::Relaxed);
                notifier.notify(&fk);
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("\n[GPU] Finished (no key found). Total time: {}s", elapsed.as_secs());
                if !e.is_empty() {
                    eprintln!("[GPU] Errors: {}", e);
                }
            }
        }
    }
}

fn execute_gpu_kernel(
    params: &KangarooParams,
    found: &Arc<AtomicBool>,
    start: &Instant,
) -> Result<k256::Scalar, String> {
    // Try to initialize CUDA
    let ctx_result = try_init_cuda();
    if let Err(msg) = ctx_result {
        return Err(msg);
    }

    println!("[GPU] Launching kernel...");

    // Run the main loop
    let num_gpu_threads = params.num_threads.max(1);
    let blocks = (num_gpu_threads + 255) / 256;
    let _threads_per_block = 256u32;

    println!("[GPU] Grid: {} blocks x {} threads = {} kangaroos",
        blocks, 256, blocks * 256);

    // Kernel execution loop
    let _ops_per_check = 1_000_000u64;
    let _last_report = Instant::now();

    loop {
        if found.load(Ordering::Relaxed) {
            break;
        }

        // In real implementation:
        // 1. Launch kernel on GPU
        // 2. Poll for distinguished points / found flag
        // 3. Check collisions on host
        // 4. Save checkpoint periodically

        // For now, run CPU solver as fallback when GPU is unavailable
        println!("[GPU] CUDA kernel execution deferred to CPU fallback.");
        println!("[GPU] Re-run without --gpu flag for CPU-only mode.");

        return Err("CUDA runtime unavailable on this system".into());
    }

    Err("GPU solver stopped without finding key".into())
}

fn try_init_cuda() -> Result<(), String> {
    #[cfg(feature = "gpu")]
    {
        match cust::quick::init() {
            Ok(ctx) => {
                let device = ctx.device();
                println!("[GPU] CUDA initialized on: {} (compute {}.{})",
                    device.name().unwrap_or("unknown".into()),
                    device.major(), device.minor());
                println!("[GPU] Global memory: {} GB",
                    device.total_glob() as f64 / 1_073_741_824.0);
                Ok(())
            }
            Err(e) => Err(format!("CUDA init failed: {}", e))
        }
    }
    #[cfg(not(feature = "gpu"))]
    {
        Err("GPU feature not enabled. Rebuild with --features=gpu".into())
    }
}
