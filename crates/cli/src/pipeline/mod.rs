//! Pipeline orchestration module.

mod orchestrator;
mod stats;

pub use orchestrator::{Pipeline, PipelineConfig};
pub use stats::PipelineStats;
