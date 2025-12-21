//! # Contracts
//!
//! 冻结的接口契约 (ICD)，定义模块间数据结构与 trait。
//! 所有业务 crate 只能依赖此 crate，禁止互相反向依赖。
//!
//! ## 时间模型
//! - 以 CARLA simulation timestamp (seconds, f64) 为主时钟
//! - `frame_id` 可选，用于排序/诊断

mod blueprint;
mod error;
mod runtime;
mod sensor;
mod sink;
mod sync;
mod sync_engine_config;

pub use blueprint::*;
pub use error::*;
pub use runtime::*;
pub use sensor::*;
pub use sink::*;
pub use sync::*;
pub use sync_engine_config::*;
