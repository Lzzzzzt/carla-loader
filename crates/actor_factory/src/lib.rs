//! # Actor Factory
//!
//! CARLA 资产工厂模块。
//!
//! 负责：
//! - 从 `WorldBlueprint` spawn vehicles 和 sensors
//! - 管理 actor 生命周期
//! - 提供 teardown 与回滚
//!
//! ## Feature Flags
//!
//! - `real-carla`: 启用真实 CARLA 客户端（需要 carla crate）

pub mod client;
pub mod error;
pub mod factory;
pub mod mock_client;

#[cfg(feature = "real-carla")]
pub mod carla_client;

pub use client::CarlaClient;
pub use contracts::{ActorId, RuntimeGraph, WorldBlueprint};
pub use error::{ActorFactoryError, Result};
pub use factory::ActorFactory;
pub use mock_client::{MockCarlaClient, MockConfig};

#[cfg(feature = "real-carla")]
pub use carla_client::RealCarlaClient;
