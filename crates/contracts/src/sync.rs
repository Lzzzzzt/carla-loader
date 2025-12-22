//! SyncedFrame - Sync Engine 输出
//!
//! 同步后的帧数据结构。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{SensorId, SensorPacket, SensorType};

/// 同步后的帧
///
/// 包含同一时间窗口内对齐的多传感器数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedFrame {
    /// 同步时间戳 (CARLA simulation time, seconds)
    pub t_sync: f64,

    /// 帧序号 (单调递增)
    pub frame_id: u64,

    /// 各传感器的数据包 (sensor_id -> packet)
    pub frames: HashMap<SensorId, SensorPacket>,

    /// 同步元信息
    pub sync_meta: SyncMeta,
}

/// 同步元信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncMeta {
    /// 参考时钟传感器 ID
    pub reference_sensor_id: SensorId,

    /// 动态窗口大小 (秒)
    pub window_size: f64,

    /// 运动强度 (0-1)，用于自适应窗口
    pub motion_intensity: Option<f64>,

    /// 各传感器的时间偏移估计 (sensor_id -> offset)
    pub time_offsets: HashMap<SensorId, f64>,

    /// 卡尔曼滤波残差 (用于自适应调参)
    pub kf_residuals: HashMap<SensorId, f64>,

    /// 缺失的传感器 (本帧无数据)
    pub missing_sensors: Vec<SensorId>,

    /// 被丢弃的包数量 (过期/乱序)
    pub dropped_count: u32,

    /// 乱序到达的包数量
    pub out_of_order_count: u32,
}

/// 同步后的数据包 (单个传感器)
#[derive(Debug, Clone)]
pub struct SyncedPacket {
    /// 原始数据包
    pub packet: SensorPacket,

    /// 校正后的时间戳
    pub corrected_timestamp: f64,

    /// 是否经过插值
    pub interpolated: bool,

    /// 原始时间戳与参考时刻的差值
    pub time_delta: f64,
}

/// 传感器缓冲区状态 (用于诊断)
#[derive(Debug, Clone, Default)]
pub struct BufferStats {
    /// 各传感器类型的缓冲深度
    pub buffer_depths: HashMap<SensorType, usize>,

    /// 总缓冲包数
    pub total_packets: usize,

    /// 最老的时间戳
    pub oldest_timestamp: Option<f64>,

    /// 最新的时间戳
    pub newest_timestamp: Option<f64>,
}
