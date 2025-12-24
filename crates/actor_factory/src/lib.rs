//! # Actor Factory
//!
//! CARLA asset factory module.
//!
//! Responsibilities:
//! - Spawn vehicles and sensors from `WorldBlueprint`
//! - Manage actor lifecycle
//! - Provide teardown and rollback
//! - Provide unified `SensorSource` abstraction
//! - Support Mock and Replay modes
//!
//! ## Feature Flags
//!
//! - `real-carla`: Enable real CARLA client (requires carla crate)

pub mod client;
pub mod error;
pub mod factory;
pub mod mock_client;
pub mod mock_sensor;
pub mod replay_sensor;

#[cfg(feature = "real-carla")]
pub mod carla_client;
#[cfg(feature = "real-carla")]
pub mod carla_sensor_source;
#[cfg(feature = "real-carla")]
pub mod sensor_data_converter;

pub use client::CarlaClient;
pub use contracts::{ActorId, RuntimeGraph, SensorSource, WorldBlueprint};
pub use error::{ActorFactoryError, Result};
pub use factory::ActorFactory;
pub use mock_client::{MockCarlaClient, MockConfig};
pub use mock_sensor::{MockSensor, MockSensorConfig};
pub use replay_sensor::{ReplayConfig, ReplaySensor};

#[cfg(feature = "real-carla")]
pub use carla_client::RealCarlaClient;
#[cfg(feature = "real-carla")]
pub use carla_sensor_source::CarlaSensorSource;
