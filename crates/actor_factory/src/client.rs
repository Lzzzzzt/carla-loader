//! CARLA 客户端抽象
//!
//! 定义与 CARLA 交互的 trait，支持真实实现和 mock 测试。

use contracts::{ActorId, Transform};

use crate::error::Result;

/// CARLA 客户端 trait
///
/// 抽象 CARLA 核心操作，便于测试和未来替换实现。
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
}

use std::future::Future;
