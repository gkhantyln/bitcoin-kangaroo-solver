use k256::{AffinePoint, EncodedPoint, ProjectivePoint, Scalar};
use k256::elliptic_curve::ops::MulByGenerator;
use k256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use k256::elliptic_curve::PrimeField;
use rand::Rng;
use sha2::{Digest, Sha256};
use ripemd::Ripemd160;

pub fn scalar_from_u64(val: u64) -> Scalar {
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&val.to_be_bytes());
    Scalar::from_repr(bytes.into()).into_option().unwrap()
}

pub fn scalar_from_bytes(bytes: &[u8; 32]) -> Scalar {
    Scalar::from_repr((*bytes).into()).into_option().unwrap()
}

pub fn scalar_to_bytes(scalar: &Scalar) -> [u8; 32] {
    scalar.to_bytes().into()
}

pub fn point_from_scalar(scalar: &Scalar) -> ProjectivePoint {
    ProjectivePoint::mul_by_generator(scalar)
}

pub fn point_to_affine_bytes(point: &ProjectivePoint) -> [u8; 33] {
    let affine = point.to_affine();
    let encoded = affine.to_encoded_point(true);
    let bytes = encoded.as_bytes();
    if bytes.len() == 1 {
        let mut result = [0u8; 33];
        result[0] = bytes[0];
        result
    } else {
        let mut result = [0u8; 33];
        result.copy_from_slice(bytes);
        result
    }
}

pub fn point_to_uncompressed_bytes(point: &ProjectivePoint) -> [u8; 65] {
    let encoded = point.to_affine().to_encoded_point(false);
    let mut result = [0u8; 65];
    result.copy_from_slice(encoded.as_bytes());
    result
}

pub fn affine_bytes_to_point(bytes: &[u8; 33]) -> Option<ProjectivePoint> {
    let encoded = EncodedPoint::from_bytes(bytes.as_slice()).ok()?;
    let affine = AffinePoint::from_encoded_point(&encoded);
    if affine.is_some().into() {
        Some(ProjectivePoint::from(affine.unwrap()))
    } else {
        None
    }
}

pub fn add_points(a: &ProjectivePoint, b: &ProjectivePoint) -> ProjectivePoint {
    *a + *b
}

pub fn subtract_points(a: &ProjectivePoint, b: &ProjectivePoint) -> ProjectivePoint {
    *a + (-*b)
}

pub fn multiply_point(scalar: &Scalar, point: &ProjectivePoint) -> ProjectivePoint {
    *point * scalar
}

pub fn compute_bitcoin_address(point: &ProjectivePoint) -> String {
    let encoded = point.to_affine().to_encoded_point(true);
    let pubkey = encoded.as_bytes();

    let sha256_hash = Sha256::digest(pubkey);
    let ripemd160_hash = Ripemd160::digest(&sha256_hash);
    let hash160 = ripemd160_hash.to_vec();

    let mut extended = Vec::with_capacity(21);
    extended.push(0x00);
    extended.extend_from_slice(&hash160);

    let checksum = Sha256::digest(&Sha256::digest(&extended));
    extended.extend_from_slice(&checksum[..4]);

    bs58::encode(&extended).into_string()
}

pub fn generate_jump_table(num_jumps: usize) -> Vec<(Scalar, ProjectivePoint)> {
    let mut table = Vec::with_capacity(num_jumps);
    let mut rng = rand::thread_rng();

    for _ in 0..num_jumps {
        let mut bytes = [0u8; 32];
        rng.fill(&mut bytes);
        let scalar = Scalar::from_repr(bytes.into()).into_option().unwrap_or(Scalar::ONE);
        let point = point_from_scalar(&scalar);
        table.push((scalar, point));
    }

    table
}

pub fn hash_to_scalar(x: &[u8; 32], jump_table_len: usize) -> usize {
    let hash = Sha256::digest(x);
    let bytes = hash.to_vec();
    let idx = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    idx % jump_table_len
}
