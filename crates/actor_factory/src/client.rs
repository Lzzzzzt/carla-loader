//! CARLA 客户端抽象
//!
//! 定义与 CARLA 交互的 trait，支持真实实现和 mock 测试。

use std::future::Future;

use contracts::{ActorId, SensorSource, SensorType, Transform};

use crate::error::Result;

/// CARLA 客户端 trait
///
/// 抽象 CARLA 核心操作，便于测试和未来替换实现。
/// 支持真实 CARLA 客户端和 Mock 客户端的统一接口。
pub trait CarlaClient: Send + Sync {
    /// 连接到 CARLA 服务器
    fn connect(&mut self, host: &str, port: u16) -> impl Future<Output = Result<()>> + Send;

    /// Spawn 车辆
    ///
    /// # Arguments
    /// * `blueprint` - 蓝图名称，如 "vehicle.tesla.model3"
    /// * `transform` - 初始位姿
    ///
    /// # Returns
    /// 新创建的 actor ID
    fn spawn_vehicle(
        &self,
        blueprint: &str,
        transform: Option<Transform>,
    ) -> impl Future<Output = Result<ActorId>> + Send;

    /// Spawn 传感器并附加到父 actor
    ///
    /// # Arguments
    /// * `blueprint` - 蓝图名称，如 "sensor.camera.rgb"
    /// * `transform` - 相对于父 actor 的位姿
    /// * `parent_id` - 父 actor ID
    /// * `attributes` - 传感器属性
    ///
    /// # Returns
    /// 新创建的 sensor actor ID
    fn spawn_sensor(
        &self,
        blueprint: &str,
        transform: Transform,
        parent_id: ActorId,
        attributes: &std::collections::HashMap<String, String>,
    ) -> impl Future<Output = Result<ActorId>> + Send;

    /// 销毁 actor
    ///
    /// 幂等操作：如果 actor 不存在，返回 Ok
    fn destroy_actor(&self, actor_id: ActorId) -> impl Future<Output = Result<()>> + Send;

    /// 检查 actor 是否存在
    fn actor_exists(&self, actor_id: ActorId) -> impl Future<Output = Result<bool>> + Send;

    /// 获取传感器数据源
    ///
    /// 返回实现 `SensorSource` 的对象，可用于 IngestionPipeline。
    /// 这是统一 Mock 和 Real 传感器的核心接口。
    ///
    /// # Arguments
    /// * `actor_id` - 传感器的 actor ID
    /// * `sensor_id` - 传感器配置 ID（用于日志和追踪）
    /// * `sensor_type` - 传感器类型
    ///
    /// # Returns
    /// 实现 `SensorSource` 的 boxed trait object，如果 actor 不存在返回 None
    fn get_sensor_source(
        &self,
        actor_id: ActorId,
        sensor_id: String,
        sensor_type: SensorType,
    ) -> Option<Box<dyn SensorSource>>;
}
