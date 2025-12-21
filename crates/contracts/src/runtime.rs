//! RuntimeGraph - Actor Factory 输出
//!
//! 运行时 actor 句柄与映射关系。

use std::collections::HashMap;

/// CARLA actor 句柄类型
pub type ActorId = u32;

/// 运行时 actor 图谱
///
/// 包含所有已创建的 CARLA actors 及其映射关系。
#[derive(Debug, Clone)]
pub struct RuntimeGraph {
    /// 车辆 ID -> Actor 句柄
    pub vehicles: HashMap<String, ActorId>,

    /// 传感器 ID -> Actor 句柄
    pub sensors: HashMap<String, ActorId>,

    /// 传感器 ID -> 所属车辆 ID
    pub sensor_to_vehicle: HashMap<String, String>,

    /// Actor 句柄 -> 配置 ID (反查)
    pub actor_to_id: HashMap<ActorId, String>,
}

impl RuntimeGraph {
    /// 创建空的 RuntimeGraph
    pub fn new() -> Self {
        Self {
            vehicles: HashMap::new(),
            sensors: HashMap::new(),
            sensor_to_vehicle: HashMap::new(),
            actor_to_id: HashMap::new(),
        }
    }

    /// 注册车辆
    pub fn register_vehicle(&mut self, id: String, actor_id: ActorId) {
        self.actor_to_id.insert(actor_id, id.clone());
        self.vehicles.insert(id, actor_id);
    }

    /// 注册传感器
    pub fn register_sensor(&mut self, sensor_id: String, vehicle_id: String, actor_id: ActorId) {
        self.actor_to_id.insert(actor_id, sensor_id.clone());
        self.sensor_to_vehicle.insert(sensor_id.clone(), vehicle_id);
        self.sensors.insert(sensor_id, actor_id);
    }

    /// 获取所有 actor 句柄 (用于 teardown)
    pub fn all_actor_ids(&self) -> Vec<ActorId> {
        self.vehicles
            .values()
            .chain(self.sensors.values())
            .copied()
            .collect()
    }
}

impl Default for RuntimeGraph {
    fn default() -> Self {
        Self::new()
    }
}
