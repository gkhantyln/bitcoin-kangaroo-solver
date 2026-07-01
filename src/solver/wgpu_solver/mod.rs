use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::path::PathBuf;

use k256::ProjectivePoint;
use k256::elliptic_curve::PrimeField;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use wgpu::util::DeviceExt;

use crate::kangaroo::{
    point,
    params::KangarooParams,
    collision::{CollisionFinder, DistPointEntry},
};
use crate::checkpoint::{Checkpoint, DistinguishedPointEntry};
use crate::notification::{FoundKey, Notify};
use crate::solver::Solver;
use crate::is_interrupted;

// ─── bytemuck-compatible GPU structs ───────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuParams {
    target_x: [u32; 8],
    target_y: [u32; 8],
    dist_bits: u32,
    table_size: u32,
    _pad: u32,
    negate_y: u32,
    dp_capacity: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuKangarooState {
    dist_x: [u32; 8],
    dist_y: [u32; 8],
    distance: [u32; 8],
    is_tame: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuJumpPoint {
    dx: [u32; 8],
    dy: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuJumpDist {
    ddist: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuDistPoint {
    x: [u32; 8],
    distance: [u32; 8],
    dist_type: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuFoundKey {
    key: [u32; 8],
}

// ─── Helpers ───────────────────────────────────────────────────────────

fn scalar_to_u32x8(s: &k256::Scalar) -> [u32; 8] {
    let bytes: [u8; 32] = s.to_bytes().into();
    let mut out = [0u32; 8];
    for i in 0..8 {
        out[i] = u32::from_le_bytes(bytes[i * 4..(i + 1) * 4].try_into().unwrap());
    }
    out
}

fn u32x8_to_scalar(v: &[u32; 8]) -> k256::Scalar {
    let mut bytes = [0u8; 32];
    for i in 0..8 {
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&v[i].to_le_bytes());
    }
    k256::Scalar::from_repr(bytes.into()).into_option().unwrap()
}

fn u32x8_to_bytes(v: &[u32; 8]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for i in 0..8 {
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&v[i].to_le_bytes());
    }
    bytes
}

fn point_to_u32x8_xy(pt: &ProjectivePoint) -> ([u32; 8], [u32; 8]) {
    let affine = pt.to_affine();
    let encoded = affine.to_encoded_point(false);
    let bytes = encoded.as_bytes();
    let x_bytes: [u8; 32] = bytes[1..33].try_into().unwrap();
    let y_bytes: [u8; 32] = bytes[33..65].try_into().unwrap();
    let mut x = [0u32; 8];
    let mut y = [0u32; 8];
    for i in 0..8 {
        x[i] = u32::from_le_bytes(x_bytes[i * 4..(i + 1) * 4].try_into().unwrap());
        y[i] = u32::from_le_bytes(y_bytes[i * 4..(i + 1) * 4].try_into().unwrap());
    }
    (x, y)
}

fn is_zero_u32x8(v: &[u32; 8]) -> bool {
    v.iter().all(|&x| x == 0)
}

// ─── GPU Solver ────────────────────────────────────────────────────────

pub struct WgpuSolver;

impl Solver for WgpuSolver {
    fn run(&self, params: &KangarooParams, checkpoint: Option<&Checkpoint>, notifier: &dyn Notify) {
        let found = Arc::new(AtomicBool::new(false));
        let total_ops = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let start = Instant::now();

        println!("\n[wgpu] Initializing GPU solver...");
        println!("[wgpu] Puzzle #{} | Distinguished bits: {}", params.puzzle_id, params.distinguished_bit);
        println!("[wgpu] Range width: 2^{}", params.puzzle_id);
        println!("[wgpu] Jump table size: {}", params.jump_table_size);
        println!("[wgpu] Negation map: {}", if params.negation_map { "ENABLED" } else { "disabled" });
        println!("[wgpu] SOTA K=1.15: {}", if params.sota_mode { "ENABLED" } else { "disabled" });

        let target_arr: [u8; 33] = params.target_point.as_slice().try_into()
            .expect("Invalid target point length");
        let target_pt = point::affine_bytes_to_point(&target_arr)
            .expect("Invalid target point");

        let (device, queue) = match init_wgpu() {
            Some(dq) => dq,
            None => {
                println!("[wgpu] No compatible GPU found.");
                return;
            }
        };

        let jump_table = if params.sota_mode {
            point::generate_sota_jump_table(params.jump_table_size)
        } else {
            point::generate_jump_table(params.jump_table_size)
        };

        let num_kangaroos = params.num_threads;
        let dp_buffer_cap = (num_kangaroos as u32 * 400).min(2_000_000);

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kangaroo shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("../../../kernels/kangaroo.wgsl"))),
        });

        let (tx, ty) = point_to_u32x8_xy(&target_pt);
        let gpu_params = GpuParams {
            target_x: tx,
            target_y: ty,
            dist_bits: params.distinguished_bit,
            table_size: params.jump_table_size as u32,
            _pad: 0,
            negate_y: if params.negation_map { 1 } else { 0 },
            dp_capacity: dp_buffer_cap,
        };

        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: bytemuck::bytes_of(&gpu_params),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let jp_data: Vec<GpuJumpPoint> = jump_table.iter().map(|(_, pt)| {
            let (dx, dy) = point_to_u32x8_xy(pt);
            GpuJumpPoint { dx, dy }
        }).collect();
        let jd_data: Vec<GpuJumpDist> = jump_table.iter().map(|(scalar, _)| {
            GpuJumpDist { ddist: scalar_to_u32x8(scalar) }
        }).collect();

        let jump_points_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("jump_points"),
            contents: bytemuck::cast_slice(&jp_data),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let jump_dists_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("jump_dists"),
            contents: bytemuck::cast_slice(&jd_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let dp_buf_size = dp_buffer_cap as usize * std::mem::size_of::<GpuDistPoint>();
        let dp_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dist_points"),
            size: dp_buf_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let dp_count_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dist_count"),
            contents: bytemuck::cast_slice(&[0u32]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        });

        let found_key_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("found_key"),
            contents: bytemuck::cast_slice(&[GpuFoundKey { key: [0u32; 8] }]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        });

        let kangaroo_buf_size = num_kangaroos as usize * std::mem::size_of::<GpuKangarooState>();
        let mut kangaroo_states = init_kangaroo_states(params, &target_pt);

        let kangaroo_input_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kangaroo_input"),
            size: kangaroo_buf_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let kangaroo_output_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kangaroo_output"),
            size: kangaroo_buf_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let dp_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dp_staging"),
            size: dp_buf_size as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dp_count_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dp_count_staging"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let found_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("found_staging"),
            size: 32,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output_staging"),
            size: kangaroo_buf_size as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kangaroo_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 6, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 7, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let pll = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kangaroo_pll"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("kangaroo_pipeline"),
            layout: Some(&pll),
            module: &shader_module,
            entry_point: "main",
            compilation_options: Default::default(),
        });

        let shared_collision = Arc::new(Mutex::new(CollisionFinder::new(target_arr)));

        if let Some(ckpt) = checkpoint {
            let mut cf = shared_collision.lock().unwrap();
            for entry in &ckpt.distinguished_points {
                cf.distinguished_points.push(DistPointEntry {
                    x_bytes: entry.x,
                    distance: entry.distance,
                    kangaroo_type: entry.kangaroo_type,
                    thread_id: entry.thread_id,
                });
            }
            total_ops.store(ckpt.total_ops as u64, Ordering::Relaxed);
            println!("[wgpu] Loaded {} DPs from checkpoint ({:?})",
                ckpt.distinguished_points.len(), params.checkpoint_path);
        }

        println!("[wgpu] Kangaroos: {} | DP buffer: {} entries", num_kangaroos, dp_buffer_cap);
        println!("[wgpu] Launching kernel...");

        let ckpt_path = params.checkpoint_path.clone();
        let mut dispatch_count: u64 = 0;
        let mut last_report = Instant::now();
        let ops_per_dispatch = 100_000u64 * num_kangaroos as u64;

        loop {
            if found.load(Ordering::Relaxed) { break; }
            if is_interrupted() {
                save_checkpoint_if_needed(params, &shared_collision, &start, total_ops.load(Ordering::Relaxed), &ckpt_path);
                println!("\n[wgpu] Interrupted.");
                return;
            }

            queue.write_buffer(&kangaroo_input_buf, 0, bytemuck::cast_slice(&kangaroo_states));
            queue.write_buffer(&dp_count_buf, 0, bytemuck::cast_slice(&[0u32]));
            queue.write_buffer(&found_key_buf, 0, bytemuck::cast_slice(&[GpuFoundKey { key: [0u32; 8] }]));

            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("kangaroo_bg"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: params_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: kangaroo_input_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: jump_points_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: jump_dists_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: dp_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: dp_count_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 6, resource: found_key_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 7, resource: kangaroo_output_buf.as_entire_binding() },
                ],
            });

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pass"), timestamp_writes: None });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bg, &[]);
                pass.dispatch_workgroups(num_kangaroos as u32, 1, 1);
            }

            encoder.copy_buffer_to_buffer(&dp_buf, 0, &dp_staging, 0, dp_buf_size as u64);
            encoder.copy_buffer_to_buffer(&dp_count_buf, 0, &dp_count_staging, 0, 4);
            encoder.copy_buffer_to_buffer(&found_key_buf, 0, &found_staging, 0, 32);
            encoder.copy_buffer_to_buffer(&kangaroo_output_buf, 0, &output_staging, 0, kangaroo_buf_size as u64);

            queue.submit(Some(encoder.finish()));
            device.poll(wgpu::Maintain::Wait);

            let dp_count = readback_u32(&device, &dp_count_staging).min(dp_buffer_cap);

            let fk_key = readback_u32x8(&device, &found_staging);
            if !is_zero_u32x8(&fk_key) {
                let scalar = u32x8_to_scalar(&fk_key);
                let pubkey = point::point_from_scalar(&scalar);
                let address = point::compute_bitcoin_address(&pubkey);
                let fk_out = FoundKey {
                    private_key: point::scalar_to_bytes(&scalar),
                    public_key: point::point_to_affine_bytes(&pubkey),
                    address,
                    puzzle_id: params.puzzle_id,
                    thread_id: 0,
                    elapsed_seconds: start.elapsed().as_secs(),
                };
                found.store(true, Ordering::Relaxed);
                notifier.notify(&fk_out);
                break;
            }

            if dp_count > 0 {
                let dp_entries = readback_dps(&device, &dp_staging, dp_count);
                let mut cf = shared_collision.lock().unwrap();
                for (x_bytes, dist_bytes, ktype) in dp_entries {
                    if let Some(result) = cf.add_point(x_bytes, dist_bytes, ktype, 0) {
                        found.store(true, Ordering::Relaxed);
                        let address = point::compute_bitcoin_address(&result.public_key_point);
                        let fk = FoundKey {
                            private_key: point::scalar_to_bytes(&result.private_key),
                            public_key: point::point_to_affine_bytes(&result.public_key_point),
                            address,
                            puzzle_id: params.puzzle_id,
                            thread_id: 0,
                            elapsed_seconds: start.elapsed().as_secs(),
                        };
                        drop(cf);
                        notifier.notify(&fk);
                        break;
                    }
                }
            }

            if found.load(Ordering::Relaxed) { break; }

            kangaroo_states = readback_kangaroo_states(&device, &output_staging, num_kangaroos);

            dispatch_count += 1;
            total_ops.fetch_add(ops_per_dispatch, Ordering::Relaxed);

            if last_report.elapsed().as_secs() >= 10 {
                let elapsed = start.elapsed().as_secs_f64();
                let rate = total_ops.load(Ordering::Relaxed) as f64 / elapsed.max(0.001);
                let dp_count_total = shared_collision.lock().unwrap().len();
                println!(
                    "[wgpu] {} dispatches | {:.2} Mops/s | {} DPs | total: {}M",
                    dispatch_count,
                    rate / 1_000_000.0,
                    dp_count_total,
                    total_ops.load(Ordering::Relaxed) / 1_000_000,
                );
                last_report = Instant::now();
            }

            if dispatch_count % 50 == 0 {
                save_checkpoint_if_needed(params, &shared_collision, &start, total_ops.load(Ordering::Relaxed), &ckpt_path);
            }
        }

        save_checkpoint_if_needed(params, &shared_collision, &start, total_ops.load(Ordering::Relaxed), &ckpt_path);

        let elapsed = start.elapsed();
        println!(
            "\n[wgpu] Finished. Dispatches: {} | Total ops: {} | Time: {}s | Rate: {:.2} Mops/s",
            dispatch_count,
            total_ops.load(Ordering::Relaxed),
            elapsed.as_secs(),
            total_ops.load(Ordering::Relaxed) as f64 / elapsed.as_secs_f64() / 1_000_000.0,
        );
    }
}

// ─── Kangaroo state init ───────────────────────────────────────────────

fn init_kangaroo_states(
    params: &KangarooParams,
    target_pt: &ProjectivePoint,
) -> Vec<GpuKangarooState> {
    let n = params.num_threads;
    let mut states = Vec::with_capacity(n);
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for i in 0..n {
        let is_tame = i % 2 == 0;
        if is_tame {
            let mut dist_bytes = [0u8; 32];
            rng.fill(&mut dist_bytes);
            let scalar = k256::Scalar::from_repr(dist_bytes.into()).into_option().unwrap();
            let pt = point::point_from_scalar(&scalar);
            let (x, y) = point_to_u32x8_xy(&pt);
            states.push(GpuKangarooState { dist_x: x, dist_y: y, distance: scalar_to_u32x8(&scalar), is_tame: 1 });
        } else {
            let (x, y) = point_to_u32x8_xy(target_pt);
            states.push(GpuKangarooState { dist_x: x, dist_y: y, distance: [0u32; 8], is_tame: 0 });
        }
    }
    states
}

// ─── Readback helpers ──────────────────────────────────────────────────

fn readback_u32(device: &wgpu::Device, staging: &wgpu::Buffer) -> u32 {
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).ok(); });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let val = u32::from_ne_bytes(data[0..4].try_into().unwrap());
    drop(data);
    staging.unmap();
    val
}

fn readback_u32x8(device: &wgpu::Device, staging: &wgpu::Buffer) -> [u32; 8] {
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).ok(); });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let mut key = [0u32; 8];
    for i in 0..8 {
        key[i] = u32::from_ne_bytes(data[i * 4..(i + 1) * 4].try_into().unwrap());
    }
    drop(data);
    staging.unmap();
    key
}

fn readback_dps(device: &wgpu::Device, staging: &wgpu::Buffer, count: u32) -> Vec<([u8; 32], [u8; 32], u8)> {
    let byte_count = count as usize * std::mem::size_of::<GpuDistPoint>();
    let slice = staging.slice(..byte_count as u64);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).ok(); });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let mut result = Vec::with_capacity(count as usize);
    let stride = std::mem::size_of::<GpuDistPoint>();
    for i in 0..count as usize {
        let offset = i * stride;
        let dp: &GpuDistPoint = bytemuck::from_bytes(&data[offset..offset + stride]);
        result.push((u32x8_to_bytes(&dp.x), u32x8_to_bytes(&dp.distance), dp.dist_type as u8));
    }
    drop(data);
    staging.unmap();
    result
}

fn readback_kangaroo_states(device: &wgpu::Device, staging: &wgpu::Buffer, count: usize) -> Vec<GpuKangarooState> {
    let byte_count = count * std::mem::size_of::<GpuKangarooState>();
    let slice = staging.slice(..byte_count as u64);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).ok(); });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let mut result = Vec::with_capacity(count);
    let stride = std::mem::size_of::<GpuKangarooState>();
    for i in 0..count {
        let offset = i * stride;
        let state: GpuKangarooState = *bytemuck::from_bytes(&data[offset..offset + stride]);
        result.push(state);
    }
    drop(data);
    staging.unmap();
    result
}

// ─── wgpu init ─────────────────────────────────────────────────────────

fn init_wgpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))?;

    println!("[wgpu] Adapter: {}", adapter.get_info().name);
    println!("[wgpu] Backend: {:?}", adapter.get_info().backend);

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("kangaroo_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
        None,
    ))
    .ok()?;

    Some((device, queue))
}

// ─── Checkpoint ────────────────────────────────────────────────────────

fn save_checkpoint_if_needed(
    params: &KangarooParams,
    collision: &Arc<Mutex<CollisionFinder>>,
    start: &Instant,
    total_ops: u64,
    ckpt_path: &Option<String>,
) {
    if let Some(ref path) = ckpt_path {
        let cf = collision.lock().unwrap();
        let ckpt = Checkpoint {
            puzzle_id: params.puzzle_id,
            start_range: params.start_range,
            end_range: params.end_range,
            target_point: params.target_point.clone(),
            distinguished_points: cf.distinguished_points.iter().map(|d| DistinguishedPointEntry {
                x: d.x_bytes,
                distance: d.distance,
                kangaroo_type: d.kangaroo_type,
                thread_id: d.thread_id,
            }).collect(),
            elapsed_seconds: start.elapsed().as_secs(),
            total_ops: total_ops as u128,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().as_secs(),
        };
        drop(cf);
        if let Err(e) = ckpt.save(&PathBuf::from(path)) {
            eprintln!("[ERROR] Checkpoint save failed: {}", e);
        }
    }
}
