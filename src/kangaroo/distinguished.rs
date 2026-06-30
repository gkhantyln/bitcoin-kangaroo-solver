use sha2::{Digest, Sha256};

pub fn is_distinguished(x_bytes: &[u8; 32], bits: u32) -> bool {
    if bits == 0 {
        return true;
    }
    let hash = Sha256::digest(x_bytes);
    let leading = u64::from_le_bytes(hash[..8].try_into().unwrap());
    let mask = if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    (leading & mask) == 0
}

pub fn check_distinguished_for_display(x_bytes: &[u8; 32], bits: u32) -> bool {
    is_distinguished(x_bytes, bits)
}
