//! Sensor adapter module
//!
//! Each adapter is responsible for converting a specific type of CARLA sensor data to `SensorPacket`.

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
