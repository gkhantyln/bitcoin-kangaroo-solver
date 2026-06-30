use k256::{ProjectivePoint, Scalar};
use crate::kangaroo::point;
use crate::kangaroo::distinguished::is_distinguished;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KangarooType {
    Tame,
    Wild,
}

pub struct KangarooWalk {
    pub kangaroo_type: KangarooType,
    pub distance: Scalar,
    pub point: ProjectivePoint,
    pub jump_table: Vec<(Scalar, ProjectivePoint)>,
    pub start_distance: Scalar,
    cached_x_bytes: Option<[u8; 32]>,
}

impl KangarooWalk {
    pub fn new(
        kangaroo_type: KangarooType,
        start_distance: Scalar,
        start_point: ProjectivePoint,
        jump_table: Vec<(Scalar, ProjectivePoint)>,
    ) -> Self {
        Self {
            kangaroo_type,
            distance: start_distance,
            point: start_point,
            jump_table,
            start_distance,
            cached_x_bytes: None,
        }
    }

    fn affine_x(&self) -> [u8; 32] {
        match self.cached_x_bytes {
            Some(bytes) => bytes,
            None => {
                let bytes = point::point_to_affine_bytes(&self.point);
                let mut x = [0u8; 32];
                x.copy_from_slice(&bytes[1..33]);
                x
            }
        }
    }

    pub fn step(&mut self) {
        let x_bytes = self.affine_x();
        let idx = point::hash_to_scalar(&x_bytes, self.jump_table.len());
        let (jump_scalar, jump_point) = &self.jump_table[idx];

        self.point = point::add_points(&self.point, jump_point);
        self.distance = self.distance + jump_scalar;

        let bytes = point::point_to_affine_bytes(&self.point);
        let mut x = [0u8; 32];
        x.copy_from_slice(&bytes[1..33]);
        self.cached_x_bytes = Some(x);
    }

    pub fn is_distinguished(&self, bits: u32) -> bool {
        let x = self.affine_x();
        is_distinguished(&x, bits)
    }

    pub fn get_x_bytes(&self) -> [u8; 32] {
        self.affine_x()
    }

    pub fn get_distance_bytes(&self) -> [u8; 32] {
        point::scalar_to_bytes(&self.distance)
    }
}