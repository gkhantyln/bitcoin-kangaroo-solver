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
