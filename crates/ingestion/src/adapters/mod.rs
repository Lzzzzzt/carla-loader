//! 传感器适配器模块
//!
//! 每个适配器负责将特定类型的 CARLA 传感器数据转换为 `SensorPacket`。

#[macro_use]
mod macros;

mod camera;
pub mod common;
mod gnss;
mod imu;
mod lidar;
mod radar;

pub use camera::CameraAdapter;
pub use gnss::GnssAdapter;
pub use imu::ImuAdapter;
pub use lidar::LidarAdapter;
pub use radar::RadarAdapter;
