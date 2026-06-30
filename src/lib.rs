use std::sync::atomic::{AtomicBool, Ordering};

pub mod kangaroo;
pub mod solver;
pub mod checkpoint;
pub mod notification;
pub mod puzzle;

pub const APP_NAME: &str = "Bitcoin Kangaroo Solver";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn is_interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

pub fn set_interrupted() {
    INTERRUPTED.store(true, Ordering::SeqCst);
}
