pub mod cpu;

#[cfg(feature = "gpu")]
pub mod gpu;

use crate::kangaroo::KangarooParams;
use crate::checkpoint::Checkpoint;
use crate::notification::Notify;

pub trait Solver: Send {
    fn run(&self, params: &KangarooParams, checkpoint: Option<&Checkpoint>, notifier: &dyn Notify);
}
