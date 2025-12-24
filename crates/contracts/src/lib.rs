//! # Contracts
//!
//! Frozen interface contracts (ICD), defining inter-module data structures and traits.
//! All business crates can only depend on this crate, reverse dependencies are prohibited.
//!
//! ## Time Model
//! - Uses CARLA simulation timestamp (seconds, f64) as primary clock
//! - `frame_id` is optional, used for ordering/diagnostics

mod blueprint;
mod error;
mod runtime;
mod sensor;
mod sensor_id;
mod sensor_source;
mod sink;
mod sync;
mod sync_engine_config;

pub use blueprint::*;
pub use error::*;
pub use runtime::*;
pub use sensor::*;
pub use sensor_id::SensorId;
pub use sensor_source::{SensorDataCallback, SensorSource};
pub use sink::*;
pub use sync::*;
pub use sync_engine_config::*;
