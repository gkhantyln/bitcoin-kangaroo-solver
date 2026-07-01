use k256::{ProjectivePoint, Scalar};
use crate::kangaroo::point;

#[derive(Clone, Debug)]
pub struct CollisionResult {
    pub tame_distance: Scalar,
    pub wild_distance: Scalar,
    pub private_key: Scalar,
    pub public_key_point: ProjectivePoint,
}

pub struct CollisionFinder {
    pub distinguished_points: Vec<DistPointEntry>,
    pub target_pubkey_bytes: [u8; 33],
}

#[derive(Clone, Debug)]
pub struct DistPointEntry {
    pub x_bytes: [u8; 32],
    pub distance: [u8; 32],
    pub kangaroo_type: u8,
    pub thread_id: u32,
}

impl CollisionFinder {
    pub fn new(target_pubkey_bytes: [u8; 33]) -> Self {
        Self { distinguished_points: Vec::new(), target_pubkey_bytes }
    }

    fn try_candidates(&self, tame_dist: &Scalar, wild_dist: &Scalar) -> Option<CollisionResult> {
        let candidates = [
            *tame_dist - *wild_dist,
            *wild_dist - *tame_dist,
            *tame_dist + *wild_dist,
            -*tame_dist - *wild_dist,
        ];
        for candidate in &candidates {
            let candidate_bytes = point::scalar_to_bytes(candidate);
            if point::verify_private_key(&candidate_bytes, &self.target_pubkey_bytes) {
                let pubkey = point::point_from_scalar(candidate);
                return Some(CollisionResult {
                    tame_distance: *tame_dist,
                    wild_distance: *wild_dist,
                    private_key: *candidate,
                    public_key_point: pubkey,
                });
            }
        }
        None
    }

    pub fn add_point(&mut self, x: [u8; 32], dist: [u8; 32], ktype: u8, tid: u32) -> Option<CollisionResult> {
        for existing in &self.distinguished_points {
            if existing.x_bytes == x && existing.kangaroo_type != ktype {
                let tame_dist = if ktype == 0 {
                    point::scalar_from_bytes(&dist)
                } else {
                    point::scalar_from_bytes(&existing.distance)
                };
                let wild_dist = if ktype == 1 {
                    point::scalar_from_bytes(&dist)
                } else {
                    point::scalar_from_bytes(&existing.distance)
                };

                if let Some(result) = self.try_candidates(&tame_dist, &wild_dist) {
                    return Some(result);
                }
            }
        }

        self.distinguished_points.push(DistPointEntry {
            x_bytes: x,
            distance: dist,
            kangaroo_type: ktype,
            thread_id: tid,
        });

        None
    }

    pub fn len(&self) -> usize {
        self.distinguished_points.len()
    }

    pub fn clear(&mut self) {
        self.distinguished_points.clear();
    }
}

impl Default for CollisionFinder {
    fn default() -> Self {
        Self::new([0x02u8; 33])
    }
}
