//! SensorSource trait - 传感器数据源抽象
//!
//! 定义传感器数据源的统一接口，解耦适配器与具体传感器实现。
//! 支持真实 CARLA 传感器和 Mock 传感器的统一处理。

use std::sync::Arc;

use crate::{SensorPacket, SensorType};

/// 传感器数据回调类型
///
/// 当传感器产生数据时，通过此回调发送 `SensorPacket`。
/// 使用 `Arc` 允许回调在多个上下文中共享。
pub type SensorDataCallback = Arc<dyn Fn(SensorPacket) + Send + Sync>;

/// 传感器数据源 trait
///
/// 抽象真实 CARLA 传感器和 Mock 传感器的共同行为。
/// 所有传感器数据源都实现此 trait，供 IngestionPipeline 使用。
///
/// # 设计原则
///
/// 1. **解耦**: 将传感器数据生成与数据消费分离
/// 2. **统一接口**: Mock 和 Real 传感器使用相同的 API
/// 3. **回调模式**: 使用回调而非通道，保持与 CARLA 原生模式一致
///
/// # 示例
///
/// ```ignore
/// let sensor: Box<dyn SensorSource> = get_sensor_source();
/// sensor.listen(Arc::new(|packet| {
///     println!("Received packet: {:?}", packet.sensor_id);
/// }));
/// // ... 使用传感器 ...
/// sensor.stop();
/// ```
pub trait SensorSource: Send + Sync {
    /// 获取传感器 ID
    fn sensor_id(&self) -> &str;

    /// 获取传感器类型
    fn sensor_type(&self) -> SensorType;

    /// 注册数据回调
    ///
    /// 当传感器产生数据时，调用回调函数发送 `SensorPacket`。
    /// 如果已经在监听，重复调用应该是幂等的（不会注册多个回调）。
    ///
    /// # Arguments
    /// * `callback` - 数据回调函数，接收 `SensorPacket`
    fn listen(&self, callback: SensorDataCallback);

    /// 停止监听
    ///
    /// 停止传感器数据生成。对于 Mock 传感器，停止后台线程；
    /// 对于 Real 传感器，调用 CARLA sensor.stop()。
    fn stop(&self);

    /// 检查是否正在监听
    fn is_listening(&self) -> bool;
}
