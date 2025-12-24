//! Mock CARLA 客户端
//!
//! 用于单元测试的 mock 实现，支持注入失败场景和回放模式。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use contracts::{ActorId, SensorSource, SensorType, Transform};
use tracing::{info, instrument};

use crate::client::CarlaClient;
use crate::error::{ActorFactoryError, Result};
use crate::mock_sensor::{MockSensor, MockSensorConfig};
use crate::replay_sensor::{ReplayConfig, ReplaySensor};

/// Mock 客户端配置
#[derive(Debug, Default, Clone)]
pub struct MockConfig {
    /// 应该失败的 vehicle IDs
    pub fail_vehicles: Vec<String>,
    /// 应该失败的 sensor IDs
    pub fail_sensors: Vec<String>,
    /// 应该失败的 destroy actor IDs
    pub fail_destroy: Vec<ActorId>,
    /// Mock 传感器配置 (用于生成模式)
    pub sensor_config: MockSensorConfig,
    /// Replay 配置 (用于回放模式)
    pub replay_config: ReplayConfig,
}

/// Mock CARLA 客户端内部状态
struct MockCarlaClientInner {
    /// 配置（可注入失败场景）
    config: MockConfig,
    /// Actor ID 计数器
    next_actor_id: AtomicU32,
    /// 已创建的 actors (actor_id -> (blueprint, sensor_type))
    actors: Mutex<HashMap<ActorId, ActorInfo>>,
    /// 连接状态
    connected: Mutex<bool>,
    /// 当前正在 spawn 的 ID（用于条件失败）
    current_spawn_id: Mutex<Option<String>>,
}

/// Mock CARLA 客户端
///
/// 使用 Arc 包装内部状态，支持 Clone。
#[derive(Clone)]
pub struct MockCarlaClient {
    inner: Arc<MockCarlaClientInner>,
}

/// Actor 信息
#[derive(Clone)]
#[allow(dead_code)] // Fields kept for debugging and potential future use
struct ActorInfo {
    blueprint: String,
    sensor_type: Option<SensorType>,
}

impl MockCarlaClient {
    /// 创建默认 mock 客户端
    pub fn new() -> Self {
        Self::with_config(MockConfig::default())
    }

    /// 使用配置创建 mock 客户端
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            inner: Arc::new(MockCarlaClientInner {
                config,
                next_actor_id: AtomicU32::new(1000), // 从 1000 开始，便于识别
                actors: Mutex::new(HashMap::new()),
                connected: Mutex::new(false),
                current_spawn_id: Mutex::new(None),
            }),
        }
    }

    /// 设置当前正在 spawn 的配置 ID
    pub fn set_current_spawn_id(&self, id: Option<String>) {
        *self.inner.current_spawn_id.lock().unwrap() = id;
    }

    /// 获取当前已创建的 actor 数量
    pub fn actor_count(&self) -> usize {
        self.inner.actors.lock().unwrap().len()
    }

    /// 获取所有已创建的 actor IDs
    pub fn all_actor_ids(&self) -> Vec<ActorId> {
        self.inner.actors.lock().unwrap().keys().copied().collect()
    }

    fn allocate_actor_id(&self) -> ActorId {
        self.inner.next_actor_id.fetch_add(1, Ordering::SeqCst)
    }

    fn should_fail_spawn(&self) -> bool {
        let current_id = self.inner.current_spawn_id.lock().unwrap();
        if let Some(id) = current_id.as_ref() {
            self.inner.config.fail_vehicles.contains(id)
                || self.inner.config.fail_sensors.contains(id)
        } else {
            false
        }
    }

    fn ensure_connected(&self) -> Result<()> {
        if *self.inner.connected.lock().unwrap() {
            Ok(())
        } else {
            Err(ActorFactoryError::ConnectionFailed {
                message: "not connected".into(),
            })
        }
    }

    /// 从 blueprint 推断传感器类型
    fn infer_sensor_type(blueprint: &str) -> Option<SensorType> {
        if blueprint.contains("camera") {
            Some(SensorType::Camera)
        } else if blueprint.contains("lidar") {
            Some(SensorType::Lidar)
        } else if blueprint.contains("imu") {
            Some(SensorType::Imu)
        } else if blueprint.contains("gnss") {
            Some(SensorType::Gnss)
        } else if blueprint.contains("radar") {
            Some(SensorType::Radar)
        } else {
            None
        }
    }
}

impl Default for MockCarlaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CarlaClient for MockCarlaClient {
    #[instrument(name = "mock_carla_connect", skip(self), fields(host = %host, port))]
    async fn connect(&mut self, host: &str, port: u16) -> Result<()> {
        let _ = (host, port);
        *self.inner.connected.lock().unwrap() = true;
        Ok(())
    }

    #[instrument(
        name = "mock_carla_spawn_vehicle",
        skip(self, transform),
        fields(blueprint = %blueprint, has_transform = transform.is_some())
    )]
    async fn spawn_vehicle(
        &self,
        blueprint: &str,
        transform: Option<Transform>,
    ) -> Result<ActorId> {
        let _ = transform;
        self.ensure_connected()?;

        if self.should_fail_spawn() {
            let id = self
                .inner
                .current_spawn_id
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_default();
            return Err(ActorFactoryError::VehicleSpawnFailed {
                vehicle_id: id,
                message: "mock failure".into(),
            });
        }

        let actor_id = self.allocate_actor_id();
        self.inner.actors.lock().unwrap().insert(
            actor_id,
            ActorInfo {
                blueprint: blueprint.to_string(),
                sensor_type: None,
            },
        );
        Ok(actor_id)
    }

    #[instrument(
        name = "mock_carla_spawn_sensor",
        skip(self, _transform, _attributes),
        fields(blueprint = %blueprint, parent_id)
    )]
    async fn spawn_sensor(
        &self,
        blueprint: &str,
        _transform: Transform,
        parent_id: ActorId,
        _attributes: &HashMap<String, String>,
    ) -> Result<ActorId> {
        self.ensure_connected()?;

        // 验证 parent 存在
        if !self.inner.actors.lock().unwrap().contains_key(&parent_id) {
            return Err(ActorFactoryError::SensorSpawnFailed {
                sensor_id: "unknown".into(),
                vehicle_id: format!("actor_{}", parent_id),
                message: "parent actor not found".into(),
            });
        }

        if self.should_fail_spawn() {
            let id = self
                .inner
                .current_spawn_id
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_default();
            return Err(ActorFactoryError::SensorSpawnFailed {
                sensor_id: id,
                vehicle_id: format!("actor_{}", parent_id),
                message: "mock failure".into(),
            });
        }

        let actor_id = self.allocate_actor_id();
        let sensor_type = Self::infer_sensor_type(blueprint);
        self.inner.actors.lock().unwrap().insert(
            actor_id,
            ActorInfo {
                blueprint: blueprint.to_string(),
                sensor_type,
            },
        );
        Ok(actor_id)
    }

    #[instrument(name = "mock_carla_destroy_actor", skip(self), fields(actor_id))]
    async fn destroy_actor(&self, actor_id: ActorId) -> Result<()> {
        if self.inner.config.fail_destroy.contains(&actor_id) {
            return Err(ActorFactoryError::DestroyFailed {
                actor_id,
                message: "mock failure".into(),
            });
        }

        // 幂等：即使不存在也返回 Ok
        self.inner.actors.lock().unwrap().remove(&actor_id);
        Ok(())
    }

    #[instrument(name = "mock_carla_actor_exists", skip(self), fields(actor_id))]
    async fn actor_exists(&self, actor_id: ActorId) -> Result<bool> {
        Ok(self.inner.actors.lock().unwrap().contains_key(&actor_id))
    }

    fn get_sensor_source(
        &self,
        actor_id: ActorId,
        sensor_id: String,
        sensor_type: SensorType,
    ) -> Option<Box<dyn SensorSource>> {
        // 验证 actor 存在
        if !self.inner.actors.lock().unwrap().contains_key(&actor_id) {
            return None;
        }

        // 如果配置了 replay_path，使用 ReplaySensor
        if let Some(ref replay_path) = self.inner.config.replay_config.replay_path {
            info!(sensor_id = %sensor_id, path = %replay_path.display(), "Using ReplaySensor");
            match ReplaySensor::load(
                replay_path,
                sensor_id.clone(),
                sensor_type,
                self.inner.config.replay_config.clone(),
            ) {
                Ok(sensor) => return Some(Box::new(sensor)),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load ReplaySensor, falling back to MockSensor");
                }
            }
        }

        // 默认使用 MockSensor 生成模式
        Some(Box::new(MockSensor::new(
            sensor_id,
            sensor_type,
            self.inner.config.sensor_config.clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{Location, Rotation};

    fn default_transform() -> Transform {
        Transform {
            location: Location {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: Rotation {
                pitch: 0.0,
                yaw: 0.0,
                roll: 0.0,
            },
        }
    }

    #[tokio::test]
    async fn test_mock_spawn_vehicle() {
        let mut client = MockCarlaClient::new();
        client.connect("localhost", 2000).await.unwrap();

        let actor_id = client
            .spawn_vehicle("vehicle.tesla.model3", None)
            .await
            .unwrap();
        assert!(actor_id >= 1000);
        assert_eq!(client.actor_count(), 1);
    }

    #[tokio::test]
    async fn test_mock_spawn_sensor() {
        let mut client = MockCarlaClient::new();
        client.connect("localhost", 2000).await.unwrap();

        let vehicle_id = client
            .spawn_vehicle("vehicle.tesla.model3", None)
            .await
            .unwrap();
        let sensor_id = client
            .spawn_sensor(
                "sensor.camera.rgb",
                default_transform(),
                vehicle_id,
                &HashMap::new(),
            )
            .await
            .unwrap();

        assert!(sensor_id > vehicle_id);
        assert_eq!(client.actor_count(), 2);
    }

    #[tokio::test]
    async fn test_mock_destroy_idempotent() {
        let mut client = MockCarlaClient::new();
        client.connect("localhost", 2000).await.unwrap();

        let actor_id = client
            .spawn_vehicle("vehicle.tesla.model3", None)
            .await
            .unwrap();
        client.destroy_actor(actor_id).await.unwrap();
        // Second destroy should also succeed
        client.destroy_actor(actor_id).await.unwrap();
        assert_eq!(client.actor_count(), 0);
    }
}
