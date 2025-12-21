//! 传感器适配器 trait

use std::sync::Arc;

use contracts::{SensorPacket, SensorType};
use tokio::sync::mpsc;

use crate::config::IngestionMetrics;

/// 传感器适配器 trait
///
/// 为每类传感器实现此 trait，负责：
/// 1. 注册 CARLA 传感器回调
/// 2. 解析传感器数据
/// 3. 封装为 `SensorPacket`
/// 4. 发送到通道（处理背压）
pub trait SensorAdapter: Send + Sync {
    /// 获取传感器 ID
    fn sensor_id(&self) -> &str;

    /// 获取传感器类型
    fn sensor_type(&self) -> SensorType;

    /// 启动传感器数据采集
    ///
    /// # Arguments
    /// * `tx` - 数据包发送通道
    /// * `metrics` - 共享的 ingestion 指标
    fn start(&self, tx: mpsc::Sender<SensorPacket>, metrics: Arc<IngestionMetrics>);

    /// 停止传感器数据采集
    fn stop(&self);

    /// 检查传感器是否正在监听
    fn is_listening(&self) -> bool;
}
