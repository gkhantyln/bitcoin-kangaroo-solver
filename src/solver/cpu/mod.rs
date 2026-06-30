use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::path::PathBuf;

use rand::Rng;
use rayon::prelude::*;

use crate::kangaroo::{
    point,
    walk::{KangarooWalk, KangarooType},
    params::KangarooParams,
    collision::DistPointEntry,
};
use crate::checkpoint::{Checkpoint, DistinguishedPointEntry};
use crate::notification::{FoundKey, Notify};
use crate::solver::Solver;
use crate::is_interrupted;

pub struct CpuSolver;

impl Solver for CpuSolver {
    fn run(&self, params: &KangarooParams, checkpoint: Option<&Checkpoint>, notifier: &dyn Notify) {
        let found = Arc::new(AtomicBool::new(false));
        let total_ops = Arc::new(AtomicU64::new(0));
        let dist_counter = Arc::new(AtomicU64::new(0));
        let start = Instant::now();

        let shared_dist_points: Arc<Mutex<Vec<DistPointEntry>>> = Arc::new(Mutex::new(Vec::new()));

        if let Some(ckpt) = checkpoint {
            for entry in &ckpt.distinguished_points {
                let mut dps = shared_dist_points.lock().unwrap();
                dps.push(DistPointEntry {
                    x_bytes: entry.x,
                    distance: entry.distance,
                    kangaroo_type: entry.kangaroo_type,
                    thread_id: entry.thread_id,
                });
                drop(dps);
                dist_counter.fetch_add(1, Ordering::Relaxed);
            }
            total_ops.store(ckpt.total_ops as u64, Ordering::Relaxed);
            println!("[CPU] Loaded {} distinguished points from checkpoint ({:?})",
                ckpt.distinguished_points.len(), params.checkpoint_path);
        }

        let jump_table = Arc::new(point::generate_jump_table(params.jump_table_size));
        let save_interval = Arc::new(120u64); // save checkpoint every 120s

        println!("\n[CPU] Starting {} threads...", params.num_threads);
        println!("[CPU] Range width: 2^{}", params.puzzle_id);
        println!("[CPU] Distinguished bits: {}", params.distinguished_bit);

        let target_arr: [u8; 33] = params.target_point.as_slice().try_into()
            .expect("Invalid target point length");
        let target_point = point::affine_bytes_to_point(&target_arr)
            .expect("Invalid target point");

        let ckpt_path = params.checkpoint_path.clone();

        (0..params.num_threads).into_par_iter().for_each(|thread_id| {
            if found.load(Ordering::Relaxed) {
                return;
            }

            let mut rng = rand::thread_rng();
            let is_tame = thread_id % 2 == 0;

            let start_dist_bytes = if let Some(_ckpt) = checkpoint {
                let mut dist = [0u8; 32];
                rng.fill(&mut dist);
                dist
            } else {
                let mut dist = [0u8; 32];
                rng.fill(&mut dist);
                dist
            };
            let start_dist = point::scalar_from_bytes(&start_dist_bytes);
            let start_pt = if is_tame {
                point::point_from_scalar(&start_dist)
            } else {
                point::subtract_points(&target_point, &point::point_from_scalar(&start_dist))
            };

            let mut walk = KangarooWalk::new(
                if is_tame { KangarooType::Tame } else { KangarooType::Wild },
                start_dist, start_pt, jump_table.as_ref().clone(),
            );
            let mut ops: u64 = 0;
            let mut last_report = Instant::now();

            loop {
                if found.load(Ordering::Relaxed) {
                    return;
                }

                if is_interrupted() {
                    found.store(true, Ordering::Relaxed);
                    if let Some(ref path) = ckpt_path {
                        let dps = shared_dist_points.lock().unwrap();
                        let elapsed = start.elapsed().as_secs();
                        let ckpt = Checkpoint {
                            puzzle_id: params.puzzle_id,
                            start_range: params.start_range,
                            end_range: params.end_range,
                            target_point: params.target_point.clone(),
                            distinguished_points: dps.iter().map(|d| DistinguishedPointEntry {
                                x: d.x_bytes,
                                distance: d.distance,
                                kangaroo_type: d.kangaroo_type,
                                thread_id: d.thread_id,
                            }).collect(),
                            elapsed_seconds: elapsed,
                            total_ops: total_ops.load(Ordering::Relaxed) as u128,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default().as_secs(),
                        };
                        drop(dps);
                        if let Err(e) = ckpt.save(&PathBuf::from(path)) {
                            eprintln!("[ERROR] Checkpoint save on interrupt failed: {}", e);
                        } else {
                            println!("\n[CPU] Interrupted. Checkpoint saved to {}", path);
                        }
                    }
                    return;
                }

                walk.step();
                ops += 1;

                if walk.is_distinguished(params.distinguished_bit) {
                    let x_bytes = walk.get_x_bytes();
                    let dist_bytes = walk.get_distance_bytes();
                    let ktype = if walk.kangaroo_type == KangarooType::Tame { 0u8 } else { 1u8 };

                    dist_counter.fetch_add(1, Ordering::Relaxed);

                    let mut dps = shared_dist_points.lock().unwrap();
                    for existing in dps.iter() {
                        if existing.x_bytes == x_bytes && existing.kangaroo_type != ktype {
                            let tame_dist_bytes = if ktype == 0 { &dist_bytes } else { &existing.distance };
                            let wild_dist_bytes = if ktype == 1 { &dist_bytes } else { &existing.distance };
                            let tame_dist = point::scalar_from_bytes(tame_dist_bytes);
                            let wild_dist = point::scalar_from_bytes(wild_dist_bytes);
                            let private_key = tame_dist - wild_dist;
                            let pubkey = point::point_from_scalar(&private_key);
                            let address = point::compute_bitcoin_address(&pubkey);

                            found.store(true, Ordering::Relaxed);

                            let fk = FoundKey {
                                private_key: point::scalar_to_bytes(&private_key),
                                public_key: point::point_to_affine_bytes(&pubkey),
                                address,
                                puzzle_id: params.puzzle_id,
                                thread_id: thread_id as u32,
                                elapsed_seconds: start.elapsed().as_secs(),
                            };
                            notifier.notify(&fk);
                            return;
                        }
                    }

                    dps.push(DistPointEntry {
                        x_bytes,
                        distance: dist_bytes,
                        kangaroo_type: ktype,
                        thread_id: thread_id as u32,
                    });
                    drop(dps);
                }

                total_ops.fetch_add(1, Ordering::Relaxed);

                if ops % 1_000_000 == 0 && last_report.elapsed().as_secs() >= 10 {
                    last_report = Instant::now();
                    let elapsed = start.elapsed().as_secs_f64();
                    let rate = total_ops.load(Ordering::Relaxed) as f64 / elapsed.max(0.001);
                    let dists = dist_counter.load(Ordering::Relaxed);
                    println!(
                        "[CPU-{}] {}M ops/s | {} distinguished | total: {}M",
                        thread_id,
                        rate / 1_000_000.0,
                        dists,
                        total_ops.load(Ordering::Relaxed) / 1_000_000,
                    );
                }

                if ops % 10_000_000 == 0 && !found.load(Ordering::Relaxed) {
                    let elapsed = start.elapsed().as_secs();
                    if elapsed > 0 && elapsed % *save_interval == 0 {
                        let dps = shared_dist_points.lock().unwrap();
                        let ckpt = Checkpoint {
                            puzzle_id: params.puzzle_id,
                            start_range: params.start_range,
                            end_range: params.end_range,
                            target_point: params.target_point.clone(),
                            distinguished_points: dps.iter().map(|d| DistinguishedPointEntry {
                                x: d.x_bytes,
                                distance: d.distance,
                                kangaroo_type: d.kangaroo_type,
                                thread_id: d.thread_id,
                            }).collect(),
                            elapsed_seconds: elapsed,
                            total_ops: total_ops.load(Ordering::Relaxed) as u128,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default().as_secs(),
                        };
                        drop(dps);
                        if let Some(ref path) = ckpt_path {
                            if let Err(e) = ckpt.save(&PathBuf::from(path)) {
                                eprintln!("[ERROR] Checkpoint save failed: {}", e);
                            }
                        }
                    }
                }
            }
        });

        let elapsed = start.elapsed();
        println!(
            "\n[CPU] Finished. Total ops: {} | Time: {}s | Rate: {:.2} Mops/s",
            total_ops.load(Ordering::Relaxed),
            elapsed.as_secs(),
            total_ops.load(Ordering::Relaxed) as f64 / elapsed.as_secs_f64() / 1_000_000.0,
        );
    }
}
