//! # Actor Factory
//!
//! CARLA 资产工厂模块。
//!
//! 负责：
//! - 从 `WorldBlueprint` spawn vehicles 和 sensors
//! - 管理 actor 生命周期
//! - 提供 teardown 与回滚
//! - 提供统一的 `SensorSource` 抽象
//! - 支持 Mock 和 Replay 模式
//!
//! ## Feature Flags
//!
//! - `real-carla`: 启用真实 CARLA 客户端（需要 carla crate）

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
