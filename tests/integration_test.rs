use bitcoin_kangaroo_solver::kangaroo::{
    point,
    walk::{KangarooWalk, KangarooType},
    collision::{CollisionFinder, DistPointEntry},
    distinguished::is_distinguished,
    params::KangarooParams,
};

#[test]
fn test_scalar_from_u64() {
    let s = point::scalar_from_u64(42);
    let bytes = point::scalar_to_bytes(&s);
    let mut expected = [0u8; 32];
    expected[31] = 42;
    assert_eq!(bytes, expected, "scalar_from_u64(42) should produce big-endian 42");
}

#[test]
fn test_scalar_roundtrip() {
    let mut original = [0u8; 32];
    original[0] = 0x12;
    original[15] = 0xAB;
    original[31] = 0xCD;
    let scalar = point::scalar_from_bytes(&original);
    let recovered = point::scalar_to_bytes(&scalar);
    assert_eq!(original, recovered, "scalar bytes should roundtrip");
}

#[test]
fn test_point_from_scalar_roundtrip() {
    let scalar = point::scalar_from_u64(12345);
    let point = point::point_from_scalar(&scalar);
    let bytes = point::point_to_affine_bytes(&point);
    assert!(bytes[0] == 0x02 || bytes[0] == 0x03, "compressed point should start with 02 or 03, got {:02x}", bytes[0]);
    let recovered = point::affine_bytes_to_point(&bytes).expect("should decode");
    let recovered_bytes = point::point_to_affine_bytes(&recovered);
    assert_eq!(bytes, recovered_bytes, "point roundtrip should match");
}

#[test]
fn test_point_addition_commutative() {
    let a = point::scalar_from_u64(7);
    let b = point::scalar_from_u64(11);
    let pa = point::point_from_scalar(&a);
    let pb = point::point_from_scalar(&b);
    let sum1 = point::add_points(&pa, &pb);
    let sum2 = point::add_points(&pb, &pa);
    let bytes1 = point::point_to_affine_bytes(&sum1);
    let bytes2 = point::point_to_affine_bytes(&sum2);
    assert_eq!(bytes1, bytes2, "point addition should be commutative");
}

#[test]
fn test_point_subtraction_identity() {
    let s = point::scalar_from_u64(999);
    let p = point::point_from_scalar(&s);
    let zero = point::subtract_points(&p, &p);
    let zero_bytes = point::point_to_affine_bytes(&zero);
    assert_eq!(zero_bytes[0], 0x00, "point - point should be point at infinity (got prefix {:02x})", zero_bytes[0]);
}

#[test]
fn test_hash_to_scalar_deterministic() {
    let x = [0xABu8; 32];
    let idx1 = point::hash_to_scalar(&x, 256);
    let idx2 = point::hash_to_scalar(&x, 256);
    assert_eq!(idx1, idx2, "hash_to_scalar should be deterministic");
    let idx3 = point::hash_to_scalar(&x, 128);
    assert!(idx3 < 128, "hash result should be bounded by table size");
}

#[test]
fn test_generate_jump_table_size() {
    let table = point::generate_jump_table(256);
    assert_eq!(table.len(), 256, "jump table should have requested size");
    for (i, (scalar, _)) in table.iter().enumerate() {
        let bytes = point::scalar_to_bytes(scalar);
        assert!(bytes.iter().any(|&b| b != 0), "jump scalar {} should be non-zero", i);
    }
}

#[test]
fn test_is_distinguished_zero_bits() {
    let x = [0xFFu8; 32];
    assert!(is_distinguished(&x, 0), "0 distinguished bits = always true");
}

#[test]
fn test_is_distinguished_all_bits() {
    let x = [0x00u8; 32];
    let d0 = is_distinguished(&x, 0);
    assert!(d0, "0 bits = always distinguished");
    let d1 = is_distinguished(&x, 64);
    assert_eq!(d1, false, "SHA256([0;32]) unlikely has 64 leading zero bits");
}

#[test]
fn test_kangaroo_walk_steps() {
    let jump_table = point::generate_jump_table(256);
    let start_dist = point::scalar_from_u64(1000);
    let start_point = point::point_from_scalar(&start_dist);
    let mut walk = KangarooWalk::new(
        KangarooType::Tame,
        start_dist,
        start_point,
        jump_table,
    );
    let initial_x = walk.get_x_bytes();
    walk.step();
    let after_x = walk.get_x_bytes();
    assert_ne!(initial_x, after_x, "walk step should change the point");
}

#[test]
fn test_kangaroo_walk_distance_increases() {
    let jump_table = point::generate_jump_table(256);
    let start_dist = point::scalar_from_u64(500);
    let start_point = point::point_from_scalar(&start_dist);
    let mut walk = KangarooWalk::new(
        KangarooType::Tame,
        start_dist,
        start_point,
        jump_table,
    );
    let dist_before = walk.get_distance_bytes();
    walk.step();
    let dist_after = walk.get_distance_bytes();
    assert_ne!(dist_before, dist_after, "walk step should change distance");
}

#[test]
fn test_collision_detection_same_point() {
    let mut cf = CollisionFinder::new([0x02u8; 33]);
    let x = [0x42u8; 32];
    let d1 = [0x01u8; 32]; // tame distance
    let d2 = [0x02u8; 32]; // wild distance
    cf.add_point(x, d1, 0, 0); // tame
    let result = cf.add_point(x, d2, 1, 1); // wild
    // These won't verify against fake target, so they return None
    assert!(result.is_none(), "non-verifying collision should not return result");
}

#[test]
fn test_no_false_collision() {
    let mut cf = CollisionFinder::new([0x02u8; 33]);
    let x1 = [0x01u8; 32];
    let x2 = [0x02u8; 32];
    cf.add_point(x1, [0x01u8; 32], 0, 0);
    let r1 = cf.add_point(x2, [0x02u8; 32], 1, 1);
    assert!(r1.is_none(), "different X should not collide");
    let r2 = cf.add_point(x2, [0x03u8; 32], 1, 2);
    assert!(r2.is_none(), "same type (both wild) should not collide even with same X");
}

#[test]
fn test_collision_finder_size() {
    let mut cf = CollisionFinder::new([0x02u8; 33]);
    assert_eq!(cf.len(), 0);
    for i in 0..10 {
        let x = [i as u8; 32];
        cf.add_point(x, [i as u8; 32], i % 2, i as u32);
    }
    assert_eq!(cf.len(), 10);
}

#[test]
fn test_bitcoin_address_derivation() {
    let scalar = point::scalar_from_u64(1);
    let point = point::point_from_scalar(&scalar);
    let compressed = point::point_to_affine_bytes(&point);
    assert_eq!(compressed[0], 0x02, "Generator should have even y (02 prefix)");
    let address = point::compute_bitcoin_address(&point);
    assert!(!address.is_empty(), "address should not be empty");
    assert!(address.starts_with('1'), "P2PKH address should start with 1");
}

#[test]
fn test_params_creation() {
    let target = [0x02u8; 33];
    let params = KangarooParams::new(66, [0u8; 32], [0u8; 32], target, 4, 20, None, 300);
    assert_eq!(params.puzzle_id, 66);
    assert_eq!(params.num_threads, 4);
    assert_eq!(params.distinguished_bit, 20);
}

#[test]
fn test_params_range_width() {
    let mut start = [0u8; 32];
    start[31] = 1; // 2^0 = 1
    let mut end = [0u8; 32];
    end[31] = 16; // 16
    let target = [0x02u8; 33];
    let params = KangarooParams::new(0, start, end, target, 1, 20, None, 300);
    assert_eq!(params.range_width(), 15);
}

#[test]
fn test_checkpoint_roundtrip() {
    use bitcoin_kangaroo_solver::checkpoint::Checkpoint;

    let tmp = std::env::temp_dir().join("test_checkpoint.bin");
    let ckpt = Checkpoint {
        puzzle_id: 66,
        start_range: [1u8; 32],
        end_range: [2u8; 32],
        target_point: vec![0x02u8; 33],
        distinguished_points: vec![
            bitcoin_kangaroo_solver::checkpoint::DistinguishedPointEntry {
                x: [0xABu8; 32],
                distance: [0xCDu8; 32],
                kangaroo_type: 0,
                thread_id: 0,
            },
        ],
        elapsed_seconds: 3600,
        total_ops: 1_000_000,
        timestamp: 1234567890,
    };
    ckpt.save(&tmp).expect("should save");
    let loaded = Checkpoint::load(&tmp).expect("should load");
    assert_eq!(loaded.puzzle_id, ckpt.puzzle_id);
    assert_eq!(loaded.total_ops, ckpt.total_ops);
    assert_eq!(loaded.distinguished_points.len(), 1);
    assert_eq!(loaded.distinguished_points[0].x, [0xABu8; 32]);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_notifier_creation() {
    use bitcoin_kangaroo_solver::notification::Notifier;
    use bitcoin_kangaroo_solver::notification::FoundKey;
    let n = Notifier::new(None, None, None);
    let fk = FoundKey {
        private_key: [0u8; 32],
        public_key: [0x02u8; 33],
        address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".into(),
        puzzle_id: 1,
        thread_id: 0,
        elapsed_seconds: 100,
    };
    n.notify(&fk);
}

#[test]
fn test_walk_tame_vs_wild_start() {
    let jump_table = point::generate_jump_table(256);
    let dist = point::scalar_from_u64(777);
    let point_tame = point::point_from_scalar(&dist);
    let mut tame = KangarooWalk::new(KangarooType::Tame, dist, point_tame, jump_table.clone());
    let zero = point::scalar_from_u64(0);
    let pt_zero = point::point_from_scalar(&zero);
    let target = point::point_from_scalar(&point::scalar_from_u64(100));
    let wild_pt = point::subtract_points(&target, &pt_zero);
    let mut wild = KangarooWalk::new(KangarooType::Wild, zero, wild_pt, jump_table);
    let tame_type = tame.kangaroo_type;
    let wild_type = wild.kangaroo_type;
    assert_eq!(tame_type, KangarooType::Tame);
    assert_eq!(wild_type, KangarooType::Wild);
    tame.step();
    wild.step();
}

#[test]
fn test_known_collision_recovery() {
    // Compute target pubkey for private key 7
    let expected_pk = point::scalar_from_u64(7);
    let target_pk_point = point::point_from_scalar(&expected_pk);
    let target_bytes = point::point_to_affine_bytes(&target_pk_point);

    let mut cf = CollisionFinder::new(target_bytes);
    let mut tame_dist = [0u8; 32];
    tame_dist[31] = 10;
    let mut wild_dist = [0u8; 32];
    wild_dist[31] = 3;
    let x = [0xABu8; 32];
    cf.add_point(x, tame_dist, 0, 0);
    let result = cf.add_point(x, wild_dist, 1, 1).expect("collision");
    let pk_bytes = point::scalar_to_bytes(&result.private_key);
    assert_eq!(pk_bytes[31], 7, "10 - 3 should equal 7");
}

#[test]
fn test_invalid_point_rejected() {
    let invalid = [0xFFu8; 33];
    let result = point::affine_bytes_to_point(&invalid);
    assert!(result.is_none(), "invalid point should be rejected");
}

#[test]
fn test_zero_distance_point() {
    let zero = point::scalar_from_u64(0);
    let pt = point::point_from_scalar(&zero);
    let bytes = point::point_to_affine_bytes(&pt);
    assert_eq!(bytes[0], 0x00, "zero scalar should give point at infinity");
}

#[test]
fn test_hash_consistency() {
    let x = [0x42u8; 32];
    let h1 = point::hash_to_scalar(&x, 1000);
    let h2 = point::hash_to_scalar(&x, 1000);
    let h3 = point::hash_to_scalar(&x, 1000);
    assert_eq!(h1, h2);
    assert_eq!(h2, h3);
    assert!(h1 < 1000);
}

#[test]
fn test_jump_large_table() {
    let table = point::generate_jump_table(65536);
    assert_eq!(table.len(), 65536);
    let mut seen = std::collections::HashSet::new();
    for (s, _p) in &table {
        let bytes = point::scalar_to_bytes(s);
        seen.insert(bytes);
    }
    let unique_ratio = seen.len() as f64 / table.len() as f64;
    assert!(unique_ratio > 0.99, "jump table should have almost all unique scalars");
}

#[test]
fn test_dist_point_equivalence() {
    use bitcoin_kangaroo_solver::checkpoint::DistinguishedPointEntry as CkptEntry;
    let x = [0x42u8; 32];
    let dist = [0x01u8; 32];
    let dp = DistPointEntry {
        x_bytes: x,
        distance: dist,
        kangaroo_type: 0,
        thread_id: 0,
    };
    let ce = CkptEntry {
        x,
        distance: dist,
        kangaroo_type: 0,
        thread_id: 0,
    };
    assert_eq!(dp.x_bytes, ce.x);
    assert_eq!(dp.distance, ce.distance);
}

use std::sync::{Arc, Mutex};
use bitcoin_kangaroo_solver::solver::Solver;

use bitcoin_kangaroo_solver::notification::Notify;

#[cfg(feature = "gpu-wgpu")]
use bitcoin_kangaroo_solver::solver::wgpu_solver::WgpuSolver;

/// Custom notifier that captures the found key into a shared mutex for test verification.
struct CaptureNotifier {
    captured: Arc<Mutex<Option<bitcoin_kangaroo_solver::notification::FoundKey>>>,
}

impl Notify for CaptureNotifier {
    fn notify(&self, key: &bitcoin_kangaroo_solver::notification::FoundKey) {
        let mut cap = self.captured.lock().unwrap();
        *cap = Some(key.clone());
    }
}

#[test]
#[ignore = "long-running: ~30 seconds for 2^14 range"]
fn test_solver_finds_key_in_small_range() {
    use rand::Rng;
    use bitcoin_kangaroo_solver::kangaroo::point;

    let mut rng = rand::thread_rng();
    let target_pk: u64 = rng.gen_range(1..10000);
    let scalar = point::scalar_from_u64(target_pk);
    let pubkey_point = point::point_from_scalar(&scalar);
    let pubkey_bytes = point::point_to_affine_bytes(&pubkey_point);

    let mut start = [0u8; 32];
    start[31] = 1;
    let mut end = [0u8; 32];
    end[30] = 0x40; // 2^14 = 16384

    let params = bitcoin_kangaroo_solver::kangaroo::KangarooParams::new(
        0, start, end, pubkey_bytes,
        2, 10, None, 300,
    );

    let captured = Arc::new(Mutex::new(None));
    let notifier = CaptureNotifier { captured: captured.clone() };

    let solver = bitcoin_kangaroo_solver::solver::cpu::CpuSolver;
    solver.run(&params, None, &notifier);

    let cap = captured.lock().unwrap();
    assert!(cap.is_some(), "Solver should find the private key");
    let fk = cap.as_ref().unwrap();
    let found_bytes = point::scalar_to_bytes(&point::scalar_from_bytes(&fk.private_key));
    let original_bytes = point::scalar_to_bytes(&scalar);
    assert_eq!(found_bytes, original_bytes, "Found key should match original");
}

#[cfg(feature = "gpu-wgpu")]
fn create_device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    })).expect("No GPU adapter found");
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
        None,
    )).expect("Should create device")
}

/// Try to compile a WGSL shader and create a compute pipeline from it.
/// `entries` specifies the pipeline layout bindings (group 0).
/// Returns Ok(()) on success, Err(msg) on failure.
/// Compile WGSL to SPIR-V binary using naga 0.20 (same version wgpu uses).
fn wgsl_to_spirv_naga20(src: &str) -> Result<Vec<u32>, String> {
    let module = naga_20::front::wgsl::parse_str(src).map_err(|e| format!("naga parse: {e:?}"))?;
    let info = naga_20::valid::Validator::new(
        naga_20::valid::ValidationFlags::all(),
        naga_20::valid::Capabilities::all(),
    ).validate(&module).map_err(|e| format!("naga validate: {e:?}"))?;
    let options = naga_20::back::spv::Options::default();
    naga_20::back::spv::write_vec(&module, &info, &options, None)
        .map_err(|e| format!("naga spv: {e:?}"))
}

/// Try to compile a WGSL shader via pre-compiled SPIR-V and create a pipeline.
/// Uses naga 0.20 to convert WGSL→SPIR-V offline, then passes it as SPIR-V to wgpu.
fn try_compile_spirv_pipeline(
    device: &wgpu::Device,
    wgsl: &str,
    label: &str,
    entries: &[wgpu::BindGroupLayoutEntry],
) -> Result<(), String> {
    let spirv = wgsl_to_spirv_naga20(wgsl)?;
    eprintln!("  [{label}] SPIR-V: {} dwords", spirv.len());

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::SpirV(std::borrow::Cow::Owned(spirv)),
    });

    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{}_bgl", label)),
        entries,
    });
    let pll = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{}_pll", label)),
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pll),
        module: &module,
        entry_point: "main",
        compilation_options: Default::default(),
    });
    Ok(())
}

fn try_compile_and_pipeline(
    device: &wgpu::Device,
    wgsl: &str,
    label: &str,
    entries: &[wgpu::BindGroupLayoutEntry],
) -> Result<(), String> {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl)),
    });

    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{}_bgl", label)),
        entries,
    });
    let pll = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{}_pll", label)),
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pll),
        module: &module,
        entry_point: "main",
        compilation_options: Default::default(),
    });
    Ok(())
}

fn bgl_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

// ── Minimal shader: empty main ──
const MINIMAL_WGSL: &str = "@compute @workgroup_size(64) fn main() {}";

// ── Gradient shader: loads/stores from 2 buffers, no workgroup memory ──
const GRADIENT_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> a: array<u32>;
@group(0) @binding(1) var<storage, read_write> b: array<u32>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    b[lid.x] = a[lid.x] * 2u;
}
"#;

// ── Helper shader: just arithmetic helpers, no workgroup memory ──
const ARITH_WGSL: &str = r#"
fn mul_32x32(a: u32, b: u32) -> vec2<u32> {
    let aL = a & 0xFFFFu;  let aH = a >> 16u;
    let bL = b & 0xFFFFu;  let bH = b >> 16u;
    let lo  = aL * bL;
    let m1  = aL * bH;
    let m2  = aH * bL;
    let hi  = aH * bH;
    let sum = (lo >> 16u) + m1;
    let r0  = (lo & 0xFFFFu) | ((sum & 0xFFFFu) << 16u);
    let r1  = hi + (sum >> 16u) + m2;
    return vec2<u32>(r0, r1);
}
fn add_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var c: u32 = 0u;
    for (var i = 0u; i < 8u; i++) {
        let s1 = (*a)[i] + (*b)[i];
        let c1 = u32(s1 < (*a)[i]);
        let s2 = s1 + c;
        let c2 = u32(s2 < s1);
        (*out)[i] = s2;
        c = c1 + c2;
    }
}
fn sub_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var borrow: u32 = 0u;
    for (var i = 0u; i < 8u; i++) {
        let d = (*a)[i] - (*b)[i] - borrow;
        borrow = u32(d > (*a)[i]);
        (*out)[i] = d;
    }
}
fn lt_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>) -> bool {
    for (var i = 7u; i < 8u; i--) {
        if ((*a)[i] < (*b)[i]) { return true; }
        if ((*a)[i] > (*b)[i]) { return false; }
    }
    return false;
}
fn mod_mul(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var lo: array<u32, 8>;
    var hi: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { lo[i] = 0u; hi[i] = 0u; }
    for (var i = 0u; i < 8u; i++) {
        var carry: u32 = 0u;
        for (var j = 0u; j < 8u; j++) {
            let prod = mul_32x32((*a)[i], (*b)[j]);
            let idx = i + j;
            if (idx < 8u) {
                let s1 = lo[idx] + prod.x; let c1 = u32(s1 < lo[idx]);
                let s2 = s1 + carry;       let c2 = u32(s2 < s1);
                lo[idx] = s2;
                let t1 = prod.y + c1;      let tc1 = u32(t1 < prod.y);
                let t2 = t1 + c2;           let tc2 = u32(t2 < t1);
                carry = t2;
                if (tc1 + tc2 > 0u && idx + 1u < 8u) { lo[idx + 1u] = lo[idx + 1u] + tc1 + tc2; }
            } else {
                let hidx = idx - 8u;
                let s1 = hi[hidx] + prod.x; let c1 = u32(s1 < hi[hidx]);
                let s2 = s1 + carry;         let c2 = u32(s2 < s1);
                hi[hidx] = s2;
                let t1 = prod.y + c1;        let tc1 = u32(t1 < prod.y);
                let t2 = t1 + c2;             let tc2 = u32(t2 < t1);
                carry = t2;
                if (tc1 + tc2 > 0u && hidx + 1u < 8u) { hi[hidx + 1u] = hi[hidx + 1u] + tc1 + tc2; }
            }
        }
        if (carry > 0u) { hi[i] = hi[i] + carry; }
    }
    // Skip reduction for this test
    for (var i = 0u; i < 8u; i++) { (*out)[i] = lo[i]; }
}
@group(0) @binding(0) var<storage, read> a: array<u32>;
@group(0) @binding(1) var<storage, read_write> b: array<u32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    var x: array<u32, 8>;
    var y: array<u32, 8>;
    var z: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { x[i] = a[gid.x * 8u + i]; y[i] = a[gid.x * 8u + i + 8u]; }
    mod_mul(&x, &y, &z);
    for (var i = 0u; i < 8u; i++) { b[gid.x * 8u + i] = z[i]; }
}
"#;

#[test]
fn test_wgpu_shader_compiles() {
    eprintln!("[TEST] Creating wgpu instance...");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    eprintln!("[TEST] Requesting adapter...");
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }));
    if adapter.is_none() {
        eprintln!("[SKIP] No compatible GPU adapter found");
        return;
    }
    let adapter = adapter.unwrap();
    eprintln!("[TEST] Adapter: {:?}", adapter.get_info());
    eprintln!("[TEST] Requesting device...");
    let (device, _queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
        None,
    )).expect("Should create device");
    eprintln!("[TEST] Device created");

    // Test 1: Minimal (0 bindings)
    eprintln!("[TEST] Trying MINIMAL...");
    try_compile_and_pipeline(&device, MINIMAL_WGSL, "minimal", &[]).unwrap();
    eprintln!("[TEST] MINIMAL OK");

    // Test 2: Gradient (2 bindings, b[0]=read_only, b[1]=read_write)
    eprintln!("[TEST] Trying GRADIENT...");
    try_compile_and_pipeline(&device, GRADIENT_WGSL, "gradient", &[
        bgl_entry(0, true),
        bgl_entry(1, false),
    ]).unwrap();
    eprintln!("[TEST] GRADIENT OK");

    // Test 3: Arithmetic helpers (2 bindings, b[0]=read_only, b[1]=read_write)
    eprintln!("[TEST] Trying ARITH...");
    try_compile_and_pipeline(&device, ARITH_WGSL, "arith", &[
        bgl_entry(0, true),
        bgl_entry(1, false),
    ]).unwrap();
    eprintln!("[TEST] ARITH OK");

    // Test 4: Workgroup memory + simple ops (no bindings needed, all internal)
    eprintln!("[TEST] Trying WORKGROUP...");
    try_compile_and_pipeline(&device, r#"
        var<workgroup> wg_x: array<u32, 64>;
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
            wg_x[lid.x] = lid.x;
            workgroupBarrier();
            let v = wg_x[lid.x];
            wg_x[lid.x] = v + 1u;
        }
    "#, "workgroup", &[]).unwrap();
    eprintln!("[TEST] WORKGROUP OK");

    // Test 5: Workgroup with array<array<u32,8>,64> + batch_invert_64-like copy loops
    eprintln!("[TEST] Trying WG_ARRAY...");
    try_compile_and_pipeline(&device, r#"
        var<workgroup> wg_px: array<array<u32, 8>, 64>;
        var<workgroup> wg_py: array<array<u32, 8>, 64>;
        var<workgroup> wg_inv: array<array<u32, 8>, 64>;

        fn mul_32x32(a: u32, b: u32) -> vec2<u32> {
            let aL = a & 0xFFFFu;  let aH = a >> 16u;
            let bL = b & 0xFFFFu;  let bH = b >> 16u;
            let lo  = aL * bL;
            let m1  = aL * bH;
            let m2  = aH * bL;
            let hi  = aH * bH;
            let sum = (lo >> 16u) + m1;
            let r0  = (lo & 0xFFFFu) | ((sum & 0xFFFFu) << 16u);
            let r1  = hi + (sum >> 16u) + m2;
            return vec2<u32>(r0, r1);
        }

        fn add_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var c: u32 = 0u;
            for (var i = 0u; i < 8u; i++) {
                let s1 = (*a)[i] + (*b)[i]; let c1 = u32(s1 < (*a)[i]);
                let s2 = s1 + c;            let c2 = u32(s2 < s1);
                (*out)[i] = s2;
                c = c1 + c2;
            }
        }

        fn sub_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var borrow: u32 = 0u;
            for (var i = 0u; i < 8u; i++) {
                let d = (*a)[i] - (*b)[i] - borrow;
                borrow = u32(d > (*a)[i]);
                (*out)[i] = d;
            }
        }

        fn lt_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>) -> bool {
            for (var i = 7u; i < 8u; i--) {
                if ((*a)[i] < (*b)[i]) { return true; }
                if ((*a)[i] > (*b)[i]) { return false; }
            }
            return false;
        }

        // Batch invert step: multiply accumulated products into wg_inv per-kangaroo
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
            var idx = lid.x;
            // copy workgroup -> function for inversion
            var px: array<u32, 8>;
            var py: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) {
                px[i] = wg_px[idx][i];
                py[i] = wg_py[idx][i];
            }
            // placeholder: just copy back
            for (var i = 0u; i < 8u; i++) {
                wg_inv[idx][i] = px[i] + py[i];
            }
        }
    "#, "wg_array", &[]).unwrap();
    eprintln!("[TEST] WG_ARRAY OK");

    // Test 6: Struct types in storage (but only array<u32> fields, no ptr-to-struct returns)
    eprintln!("[TEST] Trying STRUCTS...");
    try_compile_and_pipeline(&device, r#"
        struct Foo { x: array<u32, 8>, y: array<u32, 8> }
        struct Bar { val: array<u32, 8> }
        @group(0) @binding(0) var<storage, read> a: array<Foo>;
        @group(0) @binding(1) var<storage, read_write> b: array<Bar>;
        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
            var r: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { r[i] = a[gid.x].x[i] + a[gid.x].y[i]; }
            for (var i = 0u; i < 8u; i++) { b[gid.x].val[i] = r[i]; }
        }
    "#, "structs", &[bgl_entry(0, true), bgl_entry(1, false)]).unwrap();
    eprintln!("[TEST] STRUCTS OK");

    // Test 7: Atomics
    eprintln!("[TEST] Trying ATOMICS...");
    try_compile_and_pipeline(&device, r#"
        @group(0) @binding(0) var<storage, read_write> counter: atomic<u32>;
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
            let v = atomicAdd(&counter, 1u);
            _ = v;
        }
    "#, "atomics", &[bgl_entry(0, false)]).unwrap();
    eprintln!("[TEST] ATOMICS OK");

    // Test 8: select() built-in
    eprintln!("[TEST] Trying SELECT...");
    try_compile_and_pipeline(&device, r#"
        @group(0) @binding(0) var<storage, read> a: array<u32>;
        @group(0) @binding(1) var<storage, read_write> b: array<u32>;
        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
            b[gid.x] = select(1u, 2u, a[gid.x] > 5u);
        }
    "#, "select", &[bgl_entry(0, true), bgl_entry(1, false)]).unwrap();
    eprintln!("[TEST] SELECT OK");

    // Test 9: Function returning a struct
    eprintln!("[TEST] Trying STRUCT_RETURN...");
    try_compile_and_pipeline(&device, r#"
        struct Point { x: array<u32, 8>, y: array<u32, 8>, z: array<u32, 8> }
        fn make_point(v: u32) -> Point {
            var p: Point;
            for (var i = 0u; i < 8u; i++) { p.x[i] = v; p.y[i] = v; p.z[i] = v; }
            return p;
        }
        fn double(p: ptr<function, Point>) -> Point {
            var q: Point;
            for (var i = 0u; i < 8u; i++) { q.x[i] = (*p).x[i] + (*p).x[i]; }
            for (var i = 0u; i < 8u; i++) { q.y[i] = (*p).y[i] + (*p).y[i]; }
            for (var i = 0u; i < 8u; i++) { q.z[i] = (*p).z[i] + (*p).z[i]; }
            return q;
        }
        @group(0) @binding(0) var<storage, read> a: array<u32>;
        @group(0) @binding(1) var<storage, read_write> b: array<u32>;
        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
            var p = make_point(a[gid.x]);
            var q = double(&p);
            b[gid.x] = q.x[0];
        }
    "#, "struct_return", &[bgl_entry(0, true), bgl_entry(1, false)]).unwrap();
    eprintln!("[TEST] STRUCT_RETURN OK");

    // Test 10: mod_inv (the standalone modular inverse function — uses ptr parameters)
    eprintln!("[TEST] Trying MOD_INV...");
    try_compile_and_pipeline(&device, r#"
        var<private> P: array<u32, 8> = array<u32, 8>(
            0xFFFFFC2Fu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
            0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
        );
        fn zero_256(out: ptr<function, array<u32, 8>>) {
            for (var i = 0u; i < 8u; i++) { (*out)[i] = 0u; }
        }
        fn one_256(out: ptr<function, array<u32, 8>>) {
            zero_256(out); (*out)[0] = 1u;
        }
        fn copy_256(src: ptr<function, array<u32, 8>>, dst: ptr<function, array<u32, 8>>) {
            for (var i = 0u; i < 8u; i++) { (*dst)[i] = (*src)[i]; }
        }
        fn mul_32x32(a: u32, b: u32) -> vec2<u32> {
            let aL = a & 0xFFFFu;  let aH = a >> 16u;
            let bL = b & 0xFFFFu;  let bH = b >> 16u;
            let lo  = aL * bL; let m1  = aL * bH; let m2  = aH * bL; let hi  = aH * bH;
            let sum = (lo >> 16u) + m1;
            let r0  = (lo & 0xFFFFu) | ((sum & 0xFFFFu) << 16u);
            let r1  = hi + (sum >> 16u) + m2;
            return vec2<u32>(r0, r1);
        }
        fn add_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var c: u32 = 0u;
            for (var i = 0u; i < 8u; i++) {
                let s1 = (*a)[i] + (*b)[i]; let c1 = u32(s1 < (*a)[i]);
                let s2 = s1 + c;            let c2 = u32(s2 < s1);
                (*out)[i] = s2; c = c1 + c2;
            }
        }
        fn sub_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var borrow: u32 = 0u;
            for (var i = 0u; i < 8u; i++) {
                let d = (*a)[i] - (*b)[i] - borrow;
                borrow = u32(d > (*a)[i]); (*out)[i] = d;
            }
        }
        fn gte_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>) -> bool {
            for (var i = 7u; i < 8u; i--) {
                if ((*a)[i] > (*b)[i]) { return true; }
                if ((*a)[i] < (*b)[i]) { return false; }
            }
            return true;
        }
        fn is_zero_256(a: ptr<function, array<u32, 8>>) -> bool {
            for (var i = 0u; i < 8u; i++) { if ((*a)[i] != 0u) { return false; } }
            return true;
        }
        fn mod_reduce(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var t: array<u32, 8>; copy_256(a, &t); var p = P;
            while (gte_256(&t, &p)) { sub_256(&t, &p, &t); }
            copy_256(&t, out);
        }
        fn mod_sq(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) { mod_mul(a, a, out); }
        fn mod_mul(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var lo: array<u32, 8>; var hi: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { lo[i] = 0u; hi[i] = 0u; }
            for (var i = 0u; i < 8u; i++) {
                var carry: u32 = 0u;
                for (var j = 0u; j < 8u; j++) {
                    let prod = mul_32x32((*a)[i], (*b)[j]); let idx = i + j;
                    if (idx < 8u) {
                        let s1 = lo[idx] + prod.x; let c1 = u32(s1 < lo[idx]);
                        let s2 = s1 + carry; let c2 = u32(s2 < s1);
                        lo[idx] = s2;
                        let t1 = prod.y + c1; let tc1 = u32(t1 < prod.y);
                        let t2 = t1 + c2; let tc2 = u32(t2 < t1);
                        carry = t2;
                        if (tc1 + tc2 > 0u) { if (idx + 1u < 8u) { lo[idx + 1u] = lo[idx + 1u] + tc1 + tc2; } else { hi[0u] = hi[0u] + tc1 + tc2; } }
                    } else {
                        let hidx = idx - 8u;
                        let s1 = hi[hidx] + prod.x; let c1 = u32(s1 < hi[hidx]);
                        let s2 = s1 + carry; let c2 = u32(s2 < s1);
                        hi[hidx] = s2;
                        let t1 = prod.y + c1; let tc1 = u32(t1 < prod.y);
                        let t2 = t1 + c2; let tc2 = u32(t2 < t1);
                        carry = t2;
                        if (tc1 + tc2 > 0u && hidx + 1u < 8u) { hi[hidx + 1u] = hi[hidx + 1u] + tc1 + tc2; }
                    }
                }
                if (carry > 0u) { hi[i] = hi[i] + carry; }
            }
            if (is_zero_256(&hi)) { mod_reduce(&lo, out); return; }
            var c: u32 = 0u;
            for (var i = 0u; i < 8u; i++) {
                let prod = mul_32x32(hi[i], 977u);
                let s = prod.x + c; let c1 = u32(s < c);
                lo[i] = s; c = prod.y + c1;
            }
            var result: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { result[i] = lo[i]; }
            mod_reduce(&result, out);
        }
        fn mod_inv(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            one_256(out);
            var exp = P; exp[0] = P[0] - 2u;
            var base: array<u32, 8>; copy_256(a, &base);
            for (var i = 7u; i < 8u; i--) {
                var bit: u32 = 0x80000000u;
                while (bit != 0u) {
                    mod_sq(out, out);
                    if ((exp[i] & bit) != 0u) { mod_mul(out, &base, out); }
                    bit = bit >> 1u;
                }
            }
        }
        @group(0) @binding(0) var<storage, read> a: array<u32>;
        @group(0) @binding(1) var<storage, read_write> b: array<u32>;
        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
            var x: array<u32, 8>; var y: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { x[i] = a[gid.x * 8u + i]; y[i] = a[gid.x * 8u + i + 8u]; }
            mod_inv(&x, &y);
            for (var i = 0u; i < 8u; i++) { b[gid.x * 8u + i] = y[i]; }
        }
    "#, "mod_inv", &[bgl_entry(0, true), bgl_entry(1, false)]).unwrap();
    eprintln!("[TEST] MOD_INV OK");

    // Test 11: Just the kernel's declarations + loop body stripped to bare minimum
    eprintln!("[TEST] Trying KERNEL_BARE...");
    try_compile_and_pipeline(&device, r#"
        var<private> P: array<u32, 8> = array<u32, 8>(
            0xFFFFFC2Fu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
            0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
        );
        var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
        struct Params { target_x: array<u32, 8>, target_y: array<u32, 8>, dist_bits: u32, table_size: u32, _pad: u32, negate_y: u32, dp_capacity: u32, }
        struct KangarooInit { dist_x: array<u32, 8>, dist_y: array<u32, 8>, distance: array<u32, 8>, is_tame: u32, }
        struct JumpTablePoint { dx: array<u32, 8>, dy: array<u32, 8>, }
        struct JumpTableDist { ddist: array<u32, 8>, }
        struct DistPointOutput { x: array<u32, 8>, distance: array<u32, 8>, dist_type: u32, }
        struct FoundKeyOutput { key: array<u32, 8>, }
        @group(0) @binding(0) var<storage, read> params: Params;
        @group(0) @binding(1) var<storage, read_write> kangaroos: array<KangarooInit>;
        @group(0) @binding(2) var<storage, read> jump_points: array<JumpTablePoint>;
        @group(0) @binding(3) var<storage, read> jump_dists: array<JumpTableDist>;
        @group(0) @binding(4) var<storage, read_write> dist_points: array<DistPointOutput>;
        @group(0) @binding(5) var<storage, read_write> dist_count: atomic<u32>;
        @group(0) @binding(6) var<storage, read_write> found_key: array<FoundKeyOutput>;
        @group(0) @binding(7) var<storage, read_write> output_state: array<KangarooInit>;
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {
            var pt_x: array<u32, 8>; var pt_y: array<u32, 8>; var pt_z: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { pt_x[i] = kangaroos[gid.x].dist_x[i]; }
            for (var i = 0u; i < 8u; i++) { pt_y[i] = kangaroos[gid.x].dist_y[i]; }
            for (var i = 0u; i < 8u; i++) { pt_z[i] = 0u; } pt_z[0] = 1u;
            var dist: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { dist[i] = kangaroos[gid.x].distance[i]; }
            let is_tame = kangaroos[gid.x].is_tame != 0u;
            for (var i = 0u; i < 8u; i++) { wg_zvals[lid.x][i] = pt_z[i]; }
            workgroupBarrier();
            let h = pt_x[0] + pt_x[7];
            let ji = h % params.table_size;
            var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { jp_x[i] = jump_points[ji].dx[i]; }
            for (var i = 0u; i < 8u; i++) { jp_y[i] = jump_points[ji].dy[i]; }
            var jd_ddist: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { jd_ddist[i] = jump_dists[ji].ddist[i]; }
            let dp_idx = atomicAdd(&dist_count, 1u);
            if (dp_idx < params.dp_capacity) {
                for (var i = 0u; i < 8u; i++) { dist_points[dp_idx].x[i] = jp_x[i]; }
                dist_points[dp_idx].dist_type = select(1u, 0u, is_tame);
            }
            for (var i = 0u; i < 8u; i++) { output_state[gid.x].dist_x[i] = pt_x[i]; }
            for (var i = 0u; i < 8u; i++) { output_state[gid.x].dist_y[i] = pt_y[i]; }
            for (var i = 0u; i < 8u; i++) { output_state[gid.x].distance[i] = dist[i]; }
            output_state[gid.x].is_tame = select(0u, 1u, is_tame);
        }
    "#, "kernel_bare", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true), bgl_entry(3, true),
        bgl_entry(4, false), bgl_entry(5, false), bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KERNEL_BARE OK");

    // Test 12: KERNEL_BARE + struct Jacobian + point_double / point_add / point_add_mixed
    eprintln!("[TEST] Trying POINT_OPS...");
    try_compile_and_pipeline(&device, r#"
        var<private> P: array<u32, 8> = array<u32, 8>(
            0xFFFFFC2Fu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
            0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
        );
        var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
        var<workgroup> wg_invz: array<array<u32, 8>, 64>;
        var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
        fn zero_256(out: ptr<function, array<u32, 8>>) { for (var i = 0u; i < 8u; i++) { (*out)[i] = 0u; } }
        fn one_256(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 1u; }
        fn copy_256(src: ptr<function, array<u32, 8>>, dst: ptr<function, array<u32, 8>>) { for (var i = 0u; i < 8u; i++) { (*dst)[i] = (*src)[i]; } }
        fn mul_32x32(a: u32, b: u32) -> vec2<u32> {
            let aL = a & 0xFFFFu; let aH = a >> 16u; let bL = b & 0xFFFFu; let bH = b >> 16u;
            let lo = aL * bL; let m1 = aL * bH; let m2 = aH * bL; let hi = aH * bH;
            let sum = (lo >> 16u) + m1;
            return vec2<u32>((lo & 0xFFFFu) | ((sum & 0xFFFFu) << 16u), hi + (sum >> 16u) + m2);
        }
        fn add_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var c: u32 = 0u;
            for (var i = 0u; i < 8u; i++) { let s1 = (*a)[i] + (*b)[i]; let c1 = u32(s1 < (*a)[i]); let s2 = s1 + c; let c2 = u32(s2 < s1); (*out)[i] = s2; c = c1 + c2; }
        }
        fn sub_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var borrow: u32 = 0u;
            for (var i = 0u; i < 8u; i++) { let d = (*a)[i] - (*b)[i] - borrow; borrow = u32(d > (*a)[i]); (*out)[i] = d; }
        }
        fn lt_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>) -> bool { for (var i = 7u; i < 8u; i--) { if ((*a)[i] < (*b)[i]) { return true; } if ((*a)[i] > (*b)[i]) { return false; } } return false; }
        fn gte_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>) -> bool { return !lt_256(a, b); }
        fn is_zero_256(a: ptr<function, array<u32, 8>>) -> bool { for (var i = 0u; i < 8u; i++) { if ((*a)[i] != 0u) { return false; } } return true; }
        fn mod_reduce(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var t: array<u32, 8>; copy_256(a, &t); var p = P; while (gte_256(&t, &p)) { sub_256(&t, &p, &t); } copy_256(&t, out);
        }
        fn mod_add(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var tmp: array<u32, 8>; add_256(a, b, &tmp); mod_reduce(&tmp, out);
        }
        fn mod_sub(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var tmp: array<u32, 8>; sub_256(a, b, &tmp); var p = P;
            if (gte_256(&tmp, &p)) { sub_256(&tmp, &p, out); } else { copy_256(&tmp, out); }
        }
        fn mod_mul(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            var lo: array<u32, 8>; var hi: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { lo[i] = 0u; hi[i] = 0u; }
            for (var i = 0u; i < 8u; i++) {
                var carry: u32 = 0u;
                for (var j = 0u; j < 8u; j++) {
                    let prod = mul_32x32((*a)[i], (*b)[j]); let idx = i + j;
                    if (idx < 8u) {
                        let s1 = lo[idx] + prod.x; let c1 = u32(s1 < lo[idx]); let s2 = s1 + carry; let c2 = u32(s2 < s1); lo[idx] = s2;
                        let t1 = prod.y + c1; let tc1 = u32(t1 < prod.y); let t2 = t1 + c2; let tc2 = u32(t2 < t1); carry = t2;
                        if (tc1 + tc2 > 0u) { if (idx + 1u < 8u) { lo[idx + 1u] += tc1 + tc2; } else { hi[0u] += tc1 + tc2; } }
                    } else {
                        let hidx = idx - 8u;
                        let s1 = hi[hidx] + prod.x; let c1 = u32(s1 < hi[hidx]); let s2 = s1 + carry; let c2 = u32(s2 < s1); hi[hidx] = s2;
                        let t1 = prod.y + c1; let tc1 = u32(t1 < prod.y); let t2 = t1 + c2; let tc2 = u32(t2 < t1); carry = t2;
                        if (tc1 + tc2 > 0u && hidx + 1u < 8u) { hi[hidx + 1u] += tc1 + tc2; }
                    }
                }
                if (carry > 0u) { hi[i] += carry; }
            }
            if (is_zero_256(&hi)) { mod_reduce(&lo, out); return; }
            var c: u32 = 0u;
            for (var i = 0u; i < 8u; i++) { let prod = mul_32x32(hi[i], 977u); let s = prod.x + c; let c1 = u32(s < c); lo[i] = s; c = prod.y + c1; }
            var result: array<u32, 8>; for (var i = 0u; i < 8u; i++) { result[i] = lo[i]; }
            mod_reduce(&result, out);
        }
        fn mod_sq(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) { mod_mul(a, a, out); }
        fn mod_inv(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
            one_256(out); var exp = P; exp[0] = P[0] - 2u; var base: array<u32, 8>; copy_256(a, &base);
            for (var i = 7u; i < 8u; i--) { var bit: u32 = 0x80000000u; while (bit != 0u) { mod_sq(out, out); if ((exp[i] & bit) != 0u) { mod_mul(out, &base, out); } bit = bit >> 1u; } }
        }
        fn hash_x(x: ptr<function, array<u32, 8>>) -> u32 { var h: u32 = 0u; for (var i = 0u; i < 8u; i++) { h = h ^ (*x)[i]; } return h; }
        fn is_distinguished(x: ptr<function, array<u32, 8>>, bits: u32) -> bool { if (bits == 0u) { return true; } return (hash_x(x) & ((1u << bits) - 1u)) == 0u; }
        fn const_3(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 3u; }
        fn const_2(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 2u; }
        fn const_4(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 4u; }
        fn const_8(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 8u; }
        struct Jacobian { x: array<u32, 8>, y: array<u32, 8>, z: array<u32, 8>, }
        fn point_copy(dst: ptr<function, Jacobian>, src: ptr<function, Jacobian>) {
            for (var i = 0u; i < 8u; i++) { (*dst).x[i] = (*src).x[i]; (*dst).y[i] = (*src).y[i]; (*dst).z[i] = (*src).z[i]; }
        }
        fn point_double(p: ptr<function, Jacobian>) -> Jacobian {
            var px = (*p).x; var py = (*p).y; var pz = (*p).z;
            var three: array<u32, 8>; const_3(&three); var two: array<u32, 8>; const_2(&two);
            var four: array<u32, 8>; const_4(&four); var eight: array<u32, 8>; const_8(&eight);
            var x2: array<u32, 8>; mod_sq(&px, &x2);
            var t: array<u32, 8>; mod_mul(&x2, &three, &t);
            var y2: array<u32, 8>; mod_sq(&py, &y2);
            var y4: array<u32, 8>; mod_sq(&y2, &y4);
            var xy2: array<u32, 8>; mod_mul(&px, &y2, &xy2);
            var xy2_8: array<u32, 8>; mod_mul(&xy2, &eight, &xy2_8);
            var t2: array<u32, 8>; mod_sq(&t, &t2);
            var x3: array<u32, 8>; mod_sub(&t2, &xy2_8, &x3);
            var xy2_4: array<u32, 8>; mod_mul(&xy2, &four, &xy2_4);
            var tmp: array<u32, 8>; mod_sub(&xy2_4, &x3, &tmp);
            var tmp2: array<u32, 8>; mod_mul(&t, &tmp, &tmp2);
            var y4_8: array<u32, 8>; mod_mul(&y4, &eight, &y4_8);
            var y3: array<u32, 8>; mod_sub(&tmp2, &y4_8, &y3);
            var yz: array<u32, 8>; mod_mul(&py, &pz, &yz);
            var z3: array<u32, 8>; mod_mul(&yz, &two, &z3);
            return Jacobian(x3, y3, z3);
        }
        fn point_add(a: ptr<function, Jacobian>, b: ptr<function, Jacobian>) -> Jacobian {
            var ax = (*a).x; var ay = (*a).y; var az = (*a).z;
            var bx = (*b).x; var by = (*b).y; var bz = (*b).z;
            var z1_2: array<u32, 8>; mod_sq(&az, &z1_2);
            var z2_2: array<u32, 8>; mod_sq(&bz, &z2_2);
            var u1: array<u32, 8>; mod_mul(&ax, &z2_2, &u1);
            var u2: array<u32, 8>; mod_mul(&bx, &z1_2, &u2);
            var z1_3: array<u32, 8>; mod_mul(&z1_2, &az, &z1_3);
            var z2_3: array<u32, 8>; mod_mul(&z2_2, &bz, &z2_3);
            var s1: array<u32, 8>; mod_mul(&ay, &z2_3, &s1);
            var s2: array<u32, 8>; mod_mul(&by, &z1_3, &s2);
            var h: array<u32, 8>; mod_sub(&u2, &u1, &h);
            var r: array<u32, 8>; mod_sub(&s2, &s1, &r);
            var zero: array<u32, 8>; zero_256(&zero);
            if (is_zero_256(&h)) { if (is_zero_256(&r)) { return point_double(a); } return Jacobian(zero, zero, zero); }
            var h2: array<u32, 8>; mod_sq(&h, &h2);
            var h3: array<u32, 8>; mod_mul(&h2, &h, &h3);
            var u1h2: array<u32, 8>; mod_mul(&u1, &h2, &u1h2);
            var r2: array<u32, 8>; mod_sq(&r, &r2);
            var u1h2_2: array<u32, 8>; mod_add(&u1h2, &u1h2, &u1h2_2);
            var x3: array<u32, 8>; mod_sub(&r2, &h3, &x3);
            var x3_2: array<u32, 8>; mod_sub(&x3, &u1h2_2, &x3_2);
            var tmp_a: array<u32, 8>; mod_sub(&u1h2, &x3_2, &tmp_a);
            var r_tmp: array<u32, 8>; mod_mul(&r, &tmp_a, &r_tmp);
            var s1h3: array<u32, 8>; mod_mul(&s1, &h3, &s1h3);
            var y3: array<u32, 8>; mod_sub(&r_tmp, &s1h3, &y3);
            var hz1: array<u32, 8>; mod_mul(&h, &az, &hz1);
            var z3: array<u32, 8>; mod_mul(&hz1, &bz, &z3);
            return Jacobian(x3_2, y3, z3);
        }
        fn point_add_mixed(a: ptr<function, Jacobian>, bx: ptr<function, array<u32, 8>>, by: ptr<function, array<u32, 8>>) -> Jacobian {
            var az = (*a).z;
            var z1_2: array<u32, 8>; mod_sq(&az, &z1_2);
            var u1 = (*a).x;
            var u2: array<u32, 8>; mod_mul(bx, &z1_2, &u2);
            var z1_3: array<u32, 8>; mod_mul(&z1_2, &az, &z1_3);
            var s1 = (*a).y;
            var s2: array<u32, 8>; mod_mul(by, &z1_3, &s2);
            var h: array<u32, 8>; mod_sub(&u2, &u1, &h);
            var r: array<u32, 8>; mod_sub(&s2, &s1, &r);
            var zero: array<u32, 8>; zero_256(&zero);
            if (is_zero_256(&h)) { if (is_zero_256(&r)) { return point_double(a); } return Jacobian(zero, zero, zero); }
            var h2: array<u32, 8>; mod_sq(&h, &h2);
            var h3: array<u32, 8>; mod_mul(&h2, &h, &h3);
            var u1h2: array<u32, 8>; mod_mul(&u1, &h2, &u1h2);
            var r2: array<u32, 8>; mod_sq(&r, &r2);
            var u1h2_2: array<u32, 8>; mod_add(&u1h2, &u1h2, &u1h2_2);
            var x3: array<u32, 8>; mod_sub(&r2, &h3, &x3);
            var x3_2: array<u32, 8>; mod_sub(&x3, &u1h2_2, &x3_2);
            var tmp_p: array<u32, 8>; mod_sub(&u1h2, &x3_2, &tmp_p);
            var r_tmp: array<u32, 8>; mod_mul(&r, &tmp_p, &r_tmp);
            var s1h3: array<u32, 8>; mod_mul(&s1, &h3, &s1h3);
            var y3: array<u32, 8>; mod_sub(&r_tmp, &s1h3, &y3);
            var z3: array<u32, 8>; mod_mul(&h, &az, &z3);
            return Jacobian(x3_2, y3, z3);
        }
        struct Params { target_x: array<u32, 8>, target_y: array<u32, 8>, dist_bits: u32, table_size: u32, _pad: u32, negate_y: u32, dp_capacity: u32, }
        struct KangarooInit { dist_x: array<u32, 8>, dist_y: array<u32, 8>, distance: array<u32, 8>, is_tame: u32, }
        struct JumpTablePoint { dx: array<u32, 8>, dy: array<u32, 8>, }
        struct JumpTableDist { ddist: array<u32, 8>, }
        struct DistPointOutput { x: array<u32, 8>, distance: array<u32, 8>, dist_type: u32, }
        struct FoundKeyOutput { key: array<u32, 8>, }
        @group(0) @binding(0) var<storage, read> params: Params;
        @group(0) @binding(1) var<storage, read_write> kangaroos: array<KangarooInit>;
        @group(0) @binding(2) var<storage, read> jump_points: array<JumpTablePoint>;
        @group(0) @binding(3) var<storage, read> jump_dists: array<JumpTableDist>;
        @group(0) @binding(4) var<storage, read_write> dist_points: array<DistPointOutput>;
        @group(0) @binding(5) var<storage, read_write> dist_count: atomic<u32>;
        @group(0) @binding(6) var<storage, read_write> found_key: array<FoundKeyOutput>;
        @group(0) @binding(7) var<storage, read_write> output_state: array<KangarooInit>;
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {
            var pt_x: array<u32, 8>; var pt_y: array<u32, 8>; var pt_z: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { pt_x[i] = kangaroos[gid.x].dist_x[i]; pt_y[i] = kangaroos[gid.x].dist_y[i]; pt_z[i] = 0u; } pt_z[0] = 1u;
            var dist: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { dist[i] = kangaroos[gid.x].distance[i]; }
            // Write Z to workgroup, batch invert, affine X
            for (var i = 0u; i < 8u; i++) { wg_zvals[lid.x][i] = pt_z[i]; }
            workgroupBarrier();
            var inv_z: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { inv_z[i] = wg_invz[lid.x][i]; }
            var inv_z_sq: array<u32, 8>; mod_sq(&inv_z, &inv_z_sq);
            var x_aff: array<u32, 8>; mod_mul(&pt_x, &inv_z_sq, &x_aff);
            // Jump table
            let ji = hash_x(&x_aff) % params.table_size;
            var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { jp_x[i] = jump_points[ji].dx[i]; jp_y[i] = jump_points[ji].dy[i]; }
            // point_add_mixed - tests struct return + ptr params
            var jpt: Jacobian; jpt.x = jp_x; jpt.y = jp_y;
            for (var i = 0u; i < 8u; i++) { jpt.z[i] = 0u; } jpt.z[0] = 1u;
            var new_pt = point_add_mixed(&jpt, &jp_x, &jp_y);
            var jd: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { jd[i] = jump_dists[ji].ddist[i]; }
            mod_add(&dist, &jd, &dist);
            // DP
            if (is_distinguished(&x_aff, params.dist_bits)) {
                let dp_idx = atomicAdd(&dist_count, 1u);
                if (dp_idx < params.dp_capacity) {
                    for (var i = 0u; i < 8u; i++) { dist_points[dp_idx].x[i] = x_aff[i]; dist_points[dp_idx].distance[i] = dist[i]; }
                    dist_points[dp_idx].dist_type = select(1u, 0u, false);
                }
            }
            // Output
            for (var i = 0u; i < 8u; i++) { output_state[gid.x].dist_x[i] = new_pt.x[i]; output_state[gid.x].dist_y[i] = new_pt.y[i]; output_state[gid.x].distance[i] = dist[i]; }
            output_state[gid.x].is_tame = select(0u, 1u, true);
        }
    "#, "point_ops", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true), bgl_entry(3, true),
        bgl_entry(4, false), bgl_entry(5, false), bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] POINT_OPS OK");

    // Test 14a: Just declare local array<array<u32, 8>, 64> — no access
    eprintln!("[TEST] Trying LOCAL_ARR_DECL...");
    try_compile_and_pipeline(&device, r#"
        @compute @workgroup_size(64)
        fn main() {
            var a: array<array<u32, 8>, 64>;
            _ = a;
        }
    "#, "local_arr_decl", &[]).unwrap();
    eprintln!("[TEST] LOCAL_ARR_DECL OK");

    // Test 14b: Declare local array<array<u32, 8>, 64> and write one element
    eprintln!("[TEST] Trying LOCAL_ARR_WRITE...");
    try_compile_and_pipeline(&device, r#"
        @compute @workgroup_size(64)
        fn main() {
            var a: array<array<u32, 8>, 64>;
            a[0][0] = 1u;
        }
    "#, "local_arr_write", &[]).unwrap();
    eprintln!("[TEST] LOCAL_ARR_WRITE OK");

    // Test 14c: Declare local array<array<u32, 8>, 64> and copy from workgroup
    eprintln!("[TEST] Trying LOCAL_ARR_COPY...");
    try_compile_and_pipeline(&device, r#"
        var<workgroup> wg_data: array<array<u32, 8>, 64>;
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
            var a: array<array<u32, 8>, 64>;
            for (var i = 0u; i < 8u; i++) { a[0][i] = wg_data[lid.x][i]; }
        }
    "#, "local_arr_copy", &[]).unwrap();
    eprintln!("[TEST] LOCAL_ARR_COPY OK");

    // Test 14d: Inline copy in loop (no function call) — should pass
    eprintln!("[TEST] Trying LOOP_COPY_INLINE...");
    try_compile_and_pipeline(&device, r#"
        @compute @workgroup_size(64)
        fn main() {
            var prod: array<array<u32, 8>, 64>;
            var tmp: array<u32, 8>;
            for (var j = 0u; j < 64u; j++) {
                for (var i = 0u; i < 8u; i++) { prod[j][i] = tmp[i]; }
            }
        }
    "#, "loop_copy_inline", &[]).unwrap();
    eprintln!("[TEST] LOOP_COPY_INLINE OK");

    // Test 14e: Just `copy_256(&tmp, &prod[j])` in a loop — isolate function call with var-indexed subarray
    eprintln!("[TEST] Trying LOOP_COPY_FN...");
    try_compile_and_pipeline(&device, r#"
        @compute @workgroup_size(64)
        fn main() {
            var prod: array<array<u32, 8>, 64>;
            var tmp: array<u32, 8>;
            for (var j = 0u; j < 64u; j++) {
                for (var i = 0u; i < 8u; i++) { prod[j][i] = tmp[i]; }
            }
        }
    "#, "loop_copy_inline", &[]).unwrap();
    eprintln!("[TEST] LOOP_COPY_INLINE OK");

    // Test 14f: Workgroup-based batch_invert (no local array<array>)
    eprintln!("[TEST] Trying BATCH_INV_WG...");
    try_compile_and_pipeline(&device, r#"
        var<workgroup> wg_prod: array<array<u32, 8>, 64>;
        var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
        var<workgroup> wg_invz: array<array<u32, 8>, 64>;
        fn copy_256(src: ptr<function, array<u32, 8>>, dst: ptr<function, array<u32, 8>>) { for (var i = 0u; i < 8u; i++) { (*dst)[i] = (*src)[i]; } }
        fn batch_invert_wg() { }
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
            for (var i = 0u; i < 8u; i++) { wg_zvals[lid.x][i] = 0u; }
            workgroupBarrier();
            if (lid.x == 0u) { }
            workgroupBarrier();
        }
    "#, "batch_inv_wg", &[]).unwrap();
    eprintln!("[TEST] BATCH_INV_WG OK");

    // Test 14g: Full batch_invert_stub but with all operations inlined (no function call for copy_256)
    eprintln!("[TEST] Trying BATCH_INV_INLINE...");
    try_compile_and_pipeline(&device, r#"
        var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
        var<workgroup> wg_invz: array<array<u32, 8>, 64>;
        fn batch_invert_inline() {
            var prod: array<array<u32, 8>, 64>;
            for (var i = 0u; i < 8u; i++) { prod[0][i] = wg_zvals[0][i]; }
            for (var j = 1u; j < 64u; j++) {
                var tmp: array<u32, 8>; for (var i = 0u; i < 8u; i++) { tmp[i] = wg_zvals[j][i]; }
                for (var i = 0u; i < 8u; i++) { prod[j][i] = tmp[i]; }
            }
            var all_inv: array<u32, 8>;
            for (var i = 0u; i < 8u; i++) { all_inv[i] = 0u; }
            for (var j = 63u; j > 0u; j--) {
                for (var i = 0u; i < 8u; i++) { wg_invz[j][i] = prod[j - 1u][i]; }
                for (var i = 0u; i < 8u; i++) { all_inv[i] = wg_zvals[j][i]; }
            }
            for (var i = 0u; i < 8u; i++) { wg_invz[0][i] = all_inv[i]; }
        }
        struct KangarooInit { dist_x: array<u32, 8>, dist_y: array<u32, 8>, distance: array<u32, 8>, is_tame: u32, }
        @group(0) @binding(0) var<storage, read_write> output_state: array<KangarooInit>;
        @compute @workgroup_size(64)
        fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {
            for (var i = 0u; i < 8u; i++) { wg_zvals[lid.x][i] = 0u; }
            workgroupBarrier();
            if (lid.x == 0u) { batch_invert_inline(); }
            workgroupBarrier();
            for (var i = 0u; i < 8u; i++) { output_state[gid.x].dist_x[i] = wg_invz[lid.x][i]; }
        }
    "#, "batch_inv_inline", &[
        bgl_entry(0, false),
    ]).unwrap();
    eprintln!("[TEST] BATCH_INV_INLINE OK");

    // Test 15: Full kernel (8 bindings)
    eprintln!("[TEST] Trying KANGAROO...");
    try_compile_and_pipeline(&device, include_str!("../kernels/kangaroo.wgsl"), "kangaroo", &[
        bgl_entry(0, true),
        bgl_entry(1, false),
        bgl_entry(2, true),
        bgl_entry(3, true),
        bgl_entry(4, false),
        bgl_entry(5, false),
        bgl_entry(6, false),
        bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v3_while() {
    let (device, _queue) = create_device();
    // Use all helpers from a modified copy of full kernel with workgroup mem only in test body.
    // We construct the kernel by removing workgroup declarations from the top of kangaroo.wgsl
    // and adding them back in the test section.
    let full = include_str!("../kernels/kangaroo.wgsl");
    // Split kernel into parts: we need everything except lines 32-34 (wg memory) and lines 530+ (main fn)
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 528)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    // V3: full helpers + my own workgroup + binding decls + simplified main with while loop
    let v3 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var x: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) {{ x[i] = jp[0u].dx[i]; }}
    var step: u32 = 0u;
    while (step < 100u) {{
        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = x[i]; }}
        workgroupBarrier();
        if (lid.x == 0u) {{
            for (var i = 0u; i < 8u; i++) {{ wg_invz[0u][i] = wg_zvals[0u][i]; }}
        }}
        workgroupBarrier();
        for (var i = 0u; i < 8u; i++) {{ x[i] = wg_invz[lid.x][i]; }}
        step++;
        if (step % 10u == 0u) {{
            let _idx = atomicAdd(&dc, 1u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V3 (while+atomic, {} bytes)...", v3.len());
    try_compile_and_pipeline(&device, &v3, "kangaroo_v3", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V3 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v4_point_ops() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 528)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    // V4: helpers + IAF batch invert + point ops in while loop
    let v4 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    let k_idx = select(1u, 0u, lid.x == 0u);
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();
        if (lid.x == 0u) {{
            for (var t = 0u; t < 64u; t++) {{
                var z: array<u32, 8>;
                for (var i = 0u; i < 8u; i++) {{ z[i] = wg_zvals[t][i]; }}
                var z_inv: array<u32, 8>;
                mod_inv(&z, &z_inv);
                for (var i = 0u; i < 8u; i++) {{ wg_invz[t][i] = z_inv[i]; }}
            }}
        }}
        workgroupBarrier();
        var inv_z: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ inv_z[i] = wg_invz[lid.x][i]; }}
        var inv_z_sq: array<u32, 8>;
        mod_sq(&inv_z, &inv_z_sq);
        workgroupBarrier();
        var my_inv_z: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ my_inv_z[i] = wg_invz[lid.x][i]; }}
        var my_inv_z_sq: array<u32, 8>;
        mod_sq(&my_inv_z, &my_inv_z_sq);
        var x_aff: array<u32, 8>;
        mod_mul(&pt.x, &my_inv_z_sq, &x_aff);
        var y_aff: array<u32, 8>;
        var my_inv_z_cu: array<u32, 8>;
        mod_mul(&my_inv_z_sq, &my_inv_z, &my_inv_z_cu);
        mod_mul(&pt.y, &my_inv_z_cu, &y_aff);

        // Jump table lookup
        var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ jp_x[i] = jp[0u].dx[i]; jp_y[i] = jp[0u].dy[i]; }}

        // Point add (tame knows distance, wild moves)
        let pt2 = point_add_mixed(&pt, &jp_x, &jp_y);
        for (var i = 0u; i < 8u; i++) {{ pt.x[i] = pt2.x[i]; pt.y[i] = pt2.y[i]; pt.z[i] = pt2.z[i]; }}
        pt = point_double(&pt);

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            // DP check — write to dp buffer
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = x_aff[i]; }}
            dp[idx].distance = array<u32, 8>(step, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V4 (point ops + IAF, {} bytes)...", v4.len());
    try_compile_and_pipeline(&device, &v4, "kangaroo_v4", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V4 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v5_point_ops() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 524)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v5 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ jp_x[i] = jp[0u].dx[i]; jp_y[i] = jp[0u].dy[i]; }}

        let pt2 = point_add_mixed(&pt, &jp_x, &jp_y);
        for (var i = 0u; i < 8u; i++) {{
            pt.x[i] = pt2.x[i]; pt.y[i] = pt2.y[i]; pt.z[i] = pt2.z[i];
        }}
        pt = point_double(&pt);

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();
        if (lid.x == 0u) {{
            for (var t = 0u; t < 64u; t++) {{
                for (var i = 0u; i < 8u; i++) {{ wg_invz[t][i] = wg_zvals[t][i]; }}
            }}
        }}
        workgroupBarrier();
        for (var i = 0u; i < 8u; i++) {{ pt.z[i] = wg_invz[lid.x][i]; }}

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = pt.x[i]; }}
            dp[idx].distance = array<u32, 8>(step, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V5 (point_ops + barrier, {} bytes)...", v5.len());
    try_compile_and_pipeline(&device, &v5, "kangaroo_v5", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V5 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v6_point_double() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 524)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v6 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        pt = point_double(&pt);

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = pt.x[i]; }}
            dp[idx].distance = array<u32, 8>(step, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V6 (point_double only, {} bytes)...", v6.len());
    try_compile_and_pipeline(&device, &v6, "kangaroo_v6", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V6 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v7_point_add_mixed() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 524)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v7 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ jp_x[i] = jp[0u].dx[i]; jp_y[i] = jp[0u].dy[i]; }}

        let pt2 = point_add_mixed(&pt, &jp_x, &jp_y);
        for (var i = 0u; i < 8u; i++) {{
            pt.x[i] = pt2.x[i]; pt.y[i] = pt2.y[i]; pt.z[i] = pt2.z[i];
        }}

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = pt.x[i]; }}
            dp[idx].distance = array<u32, 8>(step, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V7 (point_add_mixed only, {} bytes)...", v7.len());
    try_compile_and_pipeline(&device, &v7, "kangaroo_v7", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V7 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v8_add_mixed_once() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 524)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v8 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    // Call point_add_mixed once before the while loop
    var jp_x0: array<u32, 8>; var jp_y0: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) {{ jp_x0[i] = jp[0u].dx[i]; jp_y0[i] = jp[0u].dy[i]; }}
    let pt_init = point_add_mixed(&pt, &jp_x0, &jp_y0);
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = pt_init.x[i]; pt.y[i] = pt_init.y[i]; pt.z[i] = pt_init.z[i]; }}

    var step: u32 = 0u;
    while (step < 100u) {{
        pt = point_double(&pt);

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = pt.x[i]; }}
            dp[idx].distance = array<u32, 8>(step, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V8 (add_mixed ONCE before loop, {} bytes)...", v8.len());
    try_compile_and_pipeline(&device, &v8, "kangaroo_v8", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V8 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v9_add_mixed_simple_body() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    // Include everything except point_add_mixed — we'll define a simpler one
    let base: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 421) || (*i >= 457 && *i <= 511) || (*i >= 517 && *i <= 524)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v9 = format!(r#"
// Simplified point_add_mixed — no early-return, no is_zero_256 branching
fn point_add_mixed(a: ptr<function, Jacobian>, bx: ptr<function, array<u32, 8>>,
                   by: ptr<function, array<u32, 8>>) -> Jacobian {{
    var az = (*a).z;
    var z1_2: array<u32, 8>;  mod_sq(&az, &z1_2);
    var u2: array<u32, 8>;    mod_mul(bx, &z1_2, &u2);
    var z1_3: array<u32, 8>;  mod_mul(&z1_2, &az, &z1_3);
    var s2: array<u32, 8>;    mod_mul(by, &z1_3, &s2);
    var h: array<u32, 8>;     mod_sub(&u2, &(*a).x, &h);
    var r: array<u32, 8>;     mod_sub(&s2, &(*a).y, &r);
    var h2: array<u32, 8>;    mod_sq(&h, &h2);
    var h3: array<u32, 8>;    mod_mul(&h2, &h, &h3);
    var u1h2: array<u32, 8>;  mod_mul(&(*a).x, &h2, &u1h2);
    var r2: array<u32, 8>;    mod_sq(&r, &r2);
    var u1h2_2: array<u32, 8>; mod_add(&u1h2, &u1h2, &u1h2_2);
    var x3: array<u32, 8>;    mod_sub(&r2, &h3, &x3);
    var x3_2: array<u32, 8>;  mod_sub(&x3, &u1h2_2, &x3_2);
    var tmp: array<u32, 8>;   mod_sub(&u1h2, &x3_2, &tmp);
    var r_tmp: array<u32, 8>; mod_mul(&r, &tmp, &r_tmp);
    var s1h3: array<u32, 8>;  mod_mul(&(*a).y, &h3, &s1h3);
    var y3: array<u32, 8>;    mod_sub(&r_tmp, &s1h3, &y3);
    var z3: array<u32, 8>;    mod_mul(&h, &az, &z3);
    return Jacobian(x3_2, y3, z3);
}}
{base}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ jp_x[i] = jp[0u].dx[i]; jp_y[i] = jp[0u].dy[i]; }}

        let pt2 = point_add_mixed(&pt, &jp_x, &jp_y);
        for (var i = 0u; i < 8u; i++) {{
            pt.x[i] = pt2.x[i]; pt.y[i] = pt2.y[i]; pt.z[i] = pt2.z[i];
        }}

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = pt.x[i]; }}
            dp[idx].distance = array<u32, 8>(step, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V9 (add_mixed simple body - no if, in loop, {} bytes)...", v9.len());
    try_compile_and_pipeline(&device, &v9, "kangaroo_v9", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V9 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v10_point_add_not_called() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 524))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    // Include point_add_mixed in preamble but NEVER call it from main
    // Only call point_double (which V6 proved safe)
    let v10 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        pt = point_double(&pt);

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = pt.x[i]; }}
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V10 (point_add_mixed defined but NOT called, {} bytes)...", v10.len());
    try_compile_and_pipeline(&device, &v10, "kangaroo_v10", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V10 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v11_max_ops() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 524))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    // Same as V5 but call point_add_mixed instead of point_double
    let v11 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jd: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dp: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dc: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> fk: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        // Only call point_add_mixed (not point_double)
        var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ jp_x[i] = jp[0u].dx[i]; jp_y[i] = jp[0u].dy[i]; }}

        let pt2 = point_add_mixed(&pt, &jp_x, &jp_y);
        for (var i = 0u; i < 8u; i++) {{
            pt.x[i] = pt2.x[i]; pt.y[i] = pt2.y[i]; pt.z[i] = pt2.z[i];
        }}

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dc, 1u);
            for (var i = 0u; i < 8u; i++) {{ dp[idx].x[i] = pt.x[i]; }}
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ os[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V11 (point_add_mixed only, no point_double, {} bytes)...", v11.len());
    try_compile_and_pipeline(&device, &v11, "kangaroo_v11", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V11 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v12_minimal_add_mixed() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| *i <= 34 || (*i >= 35 && *i <= 528))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v12 = format!(r#"
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = 1u; pt.y[i] = 2u; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var bx: array<u32, 8>; var by: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) {{ bx[i] = 3u; by[i] = 4u; }}

    let pt2 = point_add_mixed(&pt, &bx, &by);
    output_state[gid.x].dist_x[0] = pt2.x[0];
    output_state[gid.x].dist_x[1] = pt2.x[1];
    output_state[gid.x].dist_x[2] = pt2.x[2];
    output_state[gid.x].dist_x[3] = pt2.x[3];
    output_state[gid.x].dist_x[4] = pt2.x[4];
    output_state[gid.x].dist_x[5] = pt2.x[5];
    output_state[gid.x].dist_x[6] = pt2.x[6];
    output_state[gid.x].dist_x[7] = pt2.x[7];
}}
    "#);
    let v12 = format!("{}{}", preamble, v12);
    eprintln!("[TEST] Trying KANGAROO V12 (minimal add_mixed once, {} bytes)...", v12.len());
    try_compile_and_pipeline(&device, &v12, "kangaroo_v12", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V12 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
#[ignore = "requires GPU - run manually to verify E2E correctness"]
fn test_kangaroo_v13_add_mixed_loop_barrier() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| *i <= 34 || (*i >= 35 && *i <= 528))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v13 = format!(r#"
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = 1u; pt.y[i] = 2u; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var bx: array<u32, 8>; var by: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) {{ bx[i] = 3u; by[i] = 4u; }}

    var step: u32 = 0u;
    while (step < 100u) {{
        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = bx[i]; }}
        workgroupBarrier();

        let pt2 = point_add_mixed(&pt, &bx, &by);
        bx[0] = pt2.x[0]; bx[1] = pt2.x[1]; bx[2] = pt2.x[2]; bx[3] = pt2.x[3];
        bx[4] = pt2.x[4]; bx[5] = pt2.x[5]; bx[6] = pt2.x[6]; bx[7] = pt2.x[7];

        step++;
        if (step % 10u == 0u) {{
            let _idx = atomicAdd(&dist_count, 1u);
        }}
    }}
    output_state[gid.x].dist_x[0] = bx[0];
}}
    "#);
    let v13 = format!("{}{}", preamble, v13);
    eprintln!("[TEST] Trying KANGAROO V13 (add_mixed + while + barrier + atomic, {} bytes)...", v13.len());
    try_compile_and_pipeline(&device, &v13, "kangaroo_v13", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V13 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v14_add_mixed_double_loop() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| *i <= 34 || (*i >= 35 && *i <= 528))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v14 = format!(r#"
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jump_points[0u].dx[i]; pt.y[i] = jump_points[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var bx: array<u32, 8>; var by: array<u32, 8>;

    var step: u32 = 0u;
    while (step < 100u) {{
        pt = point_double(&pt);

        for (var i = 0u; i < 8u; i++) {{ bx[i] = jump_points[0u].dx[i]; by[i] = jump_points[0u].dy[i]; }}

        let pt2 = point_add_mixed(&pt, &bx, &by);
        pt.x[0] = pt2.x[0]; pt.x[1] = pt2.x[1]; pt.x[2] = pt2.x[2]; pt.x[3] = pt2.x[3];
        pt.x[4] = pt2.x[4]; pt.x[5] = pt2.x[5]; pt.x[6] = pt2.x[6]; pt.x[7] = pt2.x[7];
        pt.y[0] = pt2.y[0]; pt.y[1] = pt2.y[1]; pt.y[2] = pt2.y[2]; pt.y[3] = pt2.y[3];
        pt.y[4] = pt2.y[4]; pt.y[5] = pt2.y[5]; pt.y[6] = pt2.y[6]; pt.y[7] = pt2.y[7];
        pt.z[0] = pt2.z[0]; pt.z[1] = pt2.z[1]; pt.z[2] = pt2.z[2]; pt.z[3] = pt2.z[3];
        pt.z[4] = pt2.z[4]; pt.z[5] = pt2.z[5]; pt.z[6] = pt2.z[6]; pt.z[7] = pt2.z[7];

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dist_count, 1u);
            for (var i = 0u; i < 8u; i++) {{ dist_points[idx].x[i] = pt.x[i]; }}
        }}
    }}
    output_state[gid.x].dist_x[0] = pt.x[0];
}}
    "#);
    let v14 = format!("{}{}", preamble, v14);
    eprintln!("[TEST] Trying KANGAROO V14 (point_double + add_mixed + while + atomic, {} bytes)...", v14.len());
    try_compile_and_pipeline(&device, &v14, "kangaroo_v14", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V14 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v15_dup_buffers() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    // Use the ORIGINAL old-style filter that excludes wg vars AND only includes bindings 1-7
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| {
            *i <= 29 || (*i >= 35 && *i <= 511) || (*i >= 517 && *i <= 524)
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    let v15 = format!(r#"{preamble}
var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;
@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> k: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jp: array<JumpTablePoint>;
@group(0) @binding(7) var<storage, read_write> os: array<KangarooInit>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jp[0u].dx[i]; pt.y[i] = jp[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var bx: array<u32, 8>; var by: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) {{ bx[i] = jp[0u].dx[i]; by[i] = jp[0u].dy[i]; }}

    let pt2 = point_add_mixed(&pt, &bx, &by);
    os[gid.x].dist_x[0] = pt2.x[0];
}}
    "#);
    eprintln!("[TEST] Trying KANGAROO V15 (duplicate buffer decl, {} bytes)...", v15.len());
    try_compile_and_pipeline(&device, &v15, "kangaroo_v15", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V15 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
fn test_kangaroo_v16_for_loop_pt2() {
    let (device, _queue) = create_device();
    let full = include_str!("../kernels/kangaroo.wgsl");
    // Clean full preamble (no duplicates)
    let preamble: String = full.lines()
        .enumerate()
        .filter(|(i, _)| *i <= 34 || (*i >= 35 && *i <= 528))
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n");

    // V7's main body structure: point_add_mixed result read via for-loop into pt
    let v16 = format!(r#"
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(global_invocation_id) gid: vec3<u32>) {{
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) {{ pt.x[i] = jump_points[0u].dx[i]; pt.y[i] = jump_points[0u].dy[i]; pt.z[i] = 0u; }}
    pt.z[0] = 1u;

    var step: u32 = 0u;
    while (step < 100u) {{
        var jp_x: array<u32, 8>; var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) {{ jp_x[i] = jump_points[0u].dx[i]; jp_y[i] = jump_points[0u].dy[i]; }}

        let pt2 = point_add_mixed(&pt, &jp_x, &jp_y);
        for (var i = 0u; i < 8u; i++) {{
            pt.x[i] = pt2.x[i]; pt.y[i] = pt2.y[i]; pt.z[i] = pt2.z[i];
        }}

        for (var i = 0u; i < 8u; i++) {{ wg_zvals[lid.x][i] = pt.z[i]; }}
        workgroupBarrier();

        step++;
        if (step % 10u == 0u) {{
            let idx = atomicAdd(&dist_count, 1u);
            for (var i = 0u; i < 8u; i++) {{ dist_points[idx].x[i] = pt.x[i]; }}
            dist_points[idx].distance = array<u32, 8>(step, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
        }}
    }}
    for (var i = 0u; i < 8u; i++) {{ output_state[gid.x].dist_x[i] = pt.x[i]; }}
}}
    "#);
    let v16 = format!("{}{}", preamble, v16);
    eprintln!("[TEST] Trying KANGAROO V16 (pt2 via for-loop + while, {} bytes)...", v16.len());
    try_compile_and_pipeline(&device, &v16, "kangaroo_v16", &[
        bgl_entry(0, true), bgl_entry(1, false), bgl_entry(2, true),
        bgl_entry(3, true), bgl_entry(4, false), bgl_entry(5, false),
        bgl_entry(6, false), bgl_entry(7, false),
    ]).unwrap();
    eprintln!("[TEST] KANGAROO V16 OK");
}

#[cfg(feature = "gpu-wgpu")]
#[test]
#[ignore = "requires GPU - run manually to verify E2E correctness"]
fn test_wgpu_solver_small_range() {
    use bitcoin_kangaroo_solver::kangaroo::point;
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let target_pk: u64 = rng.gen_range(1..1000);
    let scalar = point::scalar_from_u64(target_pk);
    let pubkey_point = point::point_from_scalar(&scalar);
    let pubkey_bytes = point::point_to_affine_bytes(&pubkey_point);

    let mut start = [0u8; 32];
    start[31] = 1;
    let mut end = [0u8; 32];
    end[30] = 0x40; // 2^14 = 16384

    let params = bitcoin_kangaroo_solver::kangaroo::KangarooParams::new(
        0, start, end, pubkey_bytes,
        128, 10, None, 300,
    );

    let captured = Arc::new(Mutex::new(None));
    let notifier = CaptureNotifier { captured: captured.clone() };

    let solver = WgpuSolver;
    solver.run(&params, None, &notifier);

    let cap = captured.lock().unwrap();
    assert!(cap.is_some(), "WgpuSolver should find the private key");
    let fk = cap.as_ref().unwrap();
    let found_bytes = point::scalar_to_bytes(&point::scalar_from_bytes(&fk.private_key));
    let original_bytes = point::scalar_to_bytes(&scalar);
    assert_eq!(found_bytes, original_bytes, "Found key should match original");
}

#[test]
fn test_spirv_naga29() -> Result<(), Box<dyn std::error::Error>> {
    let src = include_str!("../kernels/kangaroo.wgsl");
    let module = naga_29::front::wgsl::parse_str(src)?;
    let info = naga_29::valid::Validator::new(
        naga_29::valid::ValidationFlags::all(),
        naga_29::valid::Capabilities::all(),
    ).validate(&module)?;
    let options = naga_29::back::spv::Options::default();
    let spv = naga_29::back::spv::write_vec(&module, &info, &options, None)?;
    assert!(!spv.is_empty());
    eprintln!("naga 29 SPIR-V: {} dwords OK", spv.len());
    Ok(())
}

#[test]
fn test_spirv_naga20() -> Result<(), Box<dyn std::error::Error>> {
    let src = include_str!("../kernels/kangaroo.wgsl");
    let module = naga_20::front::wgsl::parse_str(src)
        .map_err(|e| format!("parse: {e:?}"))?;
    let info = naga_20::valid::Validator::new(
        naga_20::valid::ValidationFlags::all(),
        naga_20::valid::Capabilities::all(),
    ).validate(&module).map_err(|e| format!("validate: {e:?}"))?;
    let options = naga_20::back::spv::Options::default();
    let spv = naga_20::back::spv::write_vec(&module, &info, &options, None)
        .map_err(|e| format!("spv: {e:?}"))?;
    assert!(!spv.is_empty());
    eprintln!("naga 0.20 SPIR-V: {} dwords OK", spv.len());
    Ok(())
}

fn naga_compile_wgsl(src: &str) -> Result<Vec<u32>, String> {
    let module = naga_29::front::wgsl::parse_str(src).map_err(|e| format!("parse: {e}"))?;
    let info = naga_29::valid::Validator::new(
        naga_29::valid::ValidationFlags::all(),
        naga_29::valid::Capabilities::all(),
    ).validate(&module).map_err(|e| format!("validate: {e}"))?;
    let options = naga_29::back::spv::Options::default();
    naga_29::back::spv::write_vec(&module, &info, &options, None)
        .map_err(|e| format!("spv: {e}"))
}

#[test]
fn test_spirv_bisect_kangaroo() {
    let full = include_str!("../kernels/kangaroo.wgsl");

    // V1: full kernel (baseline - should fail with "not cached")
    match naga_compile_wgsl(full) {
        Ok(ref v) => eprintln!("V1 FULL: OK ({} dwords)", v.len()),
        Err(ref e) => eprintln!("V1 FULL: FAIL = {e}"),
    }

    // V2: remove while loop body entirely (replace with single "return")
    // We replace `while (step < MAX_STEPS && key_is_zero) {` with `_ = key_is_zero;` and remove matching braces
    let mut lines: Vec<&str> = full.lines().collect();
    let while_line = lines.iter().position(|l| l.trim().starts_with("while (step < MAX_STEPS")).unwrap();
    let closing_line = lines.iter().position(|l| l.trim() == "}").unwrap();
    // Find the closing brace of the while loop - it's the one before the "// ── Write final state" comment
    let final_comment = lines.iter().position(|l| l.contains("Write final state")).unwrap();
    
    // Create V2: everything before while + while body but with single pass + everything after
    let v2: Vec<&str> = lines[..while_line].iter()
        .chain(&[
            "    // V2: single pass (while replaced)",
            "    if (step < MAX_STEPS && key_is_zero) {",
        ])
        .chain(lines[while_line+1..final_comment-1].iter()) // body minus the closing }
        .chain(&[
            "    }",
        ])
        .chain(lines[final_comment-1..].iter())
        .copied().collect();
    let v2_src = v2.join("\n");
    
    match naga_compile_wgsl(&v2_src) {
        Ok(ref v) => eprintln!("V2 NO_WHILE: OK ({} dwords)", v.len()),
        Err(ref e) => eprintln!("V2 NO_WHILE: FAIL = {e}"),
    }

    // V3: remove batch_invert_64 entirely (replace with identity assignment)
    let v3 = full.replace(
        "        if (local_id == 0u) { batch_invert_64(); }",
        "        if (local_id == 0u) { for (var i = 0u; i < 8u; i++) { wg_invz[local_id][i] = wg_zvals[local_id][i]; } }"
    );
    match naga_compile_wgsl(&v3) {
        Ok(ref v) => eprintln!("V3 NO_BATCH_INV: OK ({} dwords)", v.len()),
        Err(ref e) => eprintln!("V3 NO_BATCH_INV: FAIL = {e}"),
    }

    // V4: remove DP check (if step % DP_REPORT_INTERVAL == 0u)
    let mut lines4: Vec<&str> = full.lines().collect();
    let dp_check = lines4.iter().position(|l| l.trim().contains("if (step % DP_REPORT_INTERVAL == 0u)")).unwrap();
    let dp_check_end = lines4.iter().position(|l| l.trim() == "}").unwrap();
    // Actually we need to find the matching closing brace. Let me use a different approach.
    // Skip the DP check by finding it and removing lines until the next key_is_zero check
    
    eprintln!("\n--- Bisect variants defined ---");
}
