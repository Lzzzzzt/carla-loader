//! SensorPacket - Ingestion 输出
//!
//! 原始传感器数据包结构。

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::SensorType;

/// 传感器数据包
///
/// 从 CARLA 传感器回调接收的原始数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorPacket {
    /// 传感器 ID
    pub sensor_id: String,

    /// 传感器类型
    pub sensor_type: SensorType,

    /// CARLA 仿真时间戳 (seconds, f64) - 主时钟
    pub timestamp: f64,

    /// 可选的帧序号 (用于排序/诊断)
    pub frame_id: Option<u64>,

    /// 数据载荷 (零拷贝)
    pub payload: SensorPayload,
}

/// 传感器数据载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorPayload {
    /// 图像数据 (RGB/Depth/SemanticSeg)
    Image(ImageData),

    /// LiDAR 点云
    PointCloud(PointCloudData),

    /// IMU 数据
    Imu(ImuData),

    /// GNSS 数据
    Gnss(GnssData),

    /// Radar 数据
    Radar(RadarData),

    /// 原始字节 (fallback)
    Raw(Bytes),
}

/// 图像数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    /// 图像宽度
    pub width: u32,

    /// 图像高度
    pub height: u32,

    /// 像素格式
    pub format: ImageFormat,

    /// 原始像素数据
    pub data: Bytes,
}

/// 图像格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Rgb8,
    Rgba8,
    Bgra8,
    Depth,
    SemanticSeg,
}

/// LiDAR 点云数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointCloudData {
    /// 点数量
    pub num_points: u32,

    /// 每点字节数 (通常 16: x,y,z,intensity)
    pub point_stride: u32,

    /// 点云数据
    pub data: Bytes,
}

/// IMU 数据
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImuData {
    /// 加速度计 (m/s²)
    pub accelerometer: Vector3,

    /// 陀螺仪 (rad/s)
    pub gyroscope: Vector3,

    /// 指南针 (rad)
    pub compass: f64,
}

/// GNSS 数据
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GnssData {
    /// 纬度 (度)
    pub latitude: f64,

    /// 经度 (度)
    pub longitude: f64,

    /// 高度 (米)
    pub altitude: f64,
}

/// Radar 数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarData {
    /// 检测点数量
    pub num_detections: u32,

    /// 检测数据
    pub data: Bytes,
}

/// 3D 向量
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
