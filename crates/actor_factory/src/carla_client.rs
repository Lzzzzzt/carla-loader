//! 真实 CARLA 客户端实现
//!
//! 使用 carla-rust crate 连接 CARLA 服务器。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use carla::client::{ActorBase, Client, Sensor, Vehicle, World};
use carla::geom::{Location, Rotation, Transform as CarlaTransform};
use contracts::{ActorId, Transform};
use tracing::{debug, info, instrument, warn};

use crate::client::CarlaClient;
use crate::error::{ActorFactoryError, Result};

/// 真实 CARLA 客户端
///
/// 封装 carla-rust 的 Client，实现 CarlaClient trait。
/// 使用 Mutex 实现 interior mutability，允许 `&self` 方法修改 World。
#[derive(Default, Clone)]
pub struct RealCarlaClient {
    /// CARLA 客户端
    client: Arc<Mutex<Option<Client>>>,
    /// World 引用（使用 Mutex 实现 interior mutability）
    world: Arc<Mutex<Option<World>>>,
    /// 已创建的 actor 列表（用于 teardown）
    actors: Arc<Mutex<HashMap<ActorId, ActorType>>>,
}

/// Actor 类型枚举
#[derive(Clone)]
enum ActorType {
    Vehicle(Vehicle),
    Sensor(Sensor),
}

impl RealCarlaClient {
    /// 创建新的客户端（未连接状态）
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            world: Arc::new(Mutex::new(None)),
            actors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 以 MUT 引用访问 World，确保已连接
    fn with_world_mut<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut World) -> Result<R>,
    {
        let mut world_guard = self.world.lock().unwrap();
        let world = world_guard
            .as_mut()
            .ok_or_else(|| ActorFactoryError::ConnectionFailed {
                message: "not connected to CARLA server".into(),
            })?;
        f(world)
    }

    /// 将 actor 保存到 registry，便于 teardown
    fn store_actor(&self, actor_id: ActorId, actor: ActorType) {
        self.actors.lock().unwrap().insert(actor_id, actor);
    }

    fn select_vehicle_transform(
        world: &mut World,
        blueprint: &str,
        transform: Option<Transform>,
    ) -> CarlaTransform {
        Self::to_carla_transform(transform).unwrap_or_else(|| {
            let transform = world
                .map()
                .recommended_spawn_points()
                .get(0)
                .cloned()
                .expect("no recommended spawn points");
            info!(vehicle_blueprint = blueprint, point = ?transform.location, "using default spawn point");
            transform
        })
    }

    fn create_vehicle(
        world: &mut World,
        blueprint: &str,
        transform: Option<Transform>,
    ) -> Result<Vehicle> {
        let bp_library = world.blueprint_library();
        let vehicle_bp =
            bp_library
                .find(blueprint)
                .ok_or_else(|| ActorFactoryError::VehicleSpawnFailed {
                    vehicle_id: blueprint.to_string(),
                    message: format!("blueprint '{}' not found", blueprint),
                })?;

        let carla_transform = Self::select_vehicle_transform(world, blueprint, transform);
        let actor = world
            .spawn_actor(&vehicle_bp, &carla_transform)
            .map_err(|e| ActorFactoryError::VehicleSpawnFailed {
                vehicle_id: blueprint.to_string(),
                message: e.to_string(),
            })?;

        Vehicle::try_from(actor).map_err(|_| ActorFactoryError::VehicleSpawnFailed {
            vehicle_id: blueprint.to_string(),
            message: "spawned actor is not a vehicle".to_string(),
        })
    }

    fn parent_vehicle_for_sensor(
        &self,
        sensor_blueprint: &str,
        parent_id: ActorId,
    ) -> Result<Vehicle> {
        let actors = self.actors.lock().unwrap();
        match actors.get(&parent_id) {
            Some(ActorType::Vehicle(v)) => Ok(v.clone()),
            _ => Err(ActorFactoryError::SensorSpawnFailed {
                sensor_id: sensor_blueprint.to_string(),
                vehicle_id: format!("actor_{}", parent_id),
                message: "parent vehicle not found".to_string(),
            }),
        }
    }

    fn create_sensor(
        world: &mut World,
        blueprint: &str,
        transform: Transform,
        parent_actor: &Vehicle,
        parent_id: ActorId,
        attributes: &HashMap<String, String>,
    ) -> Result<Sensor> {
        let bp_library = world.blueprint_library();
        let mut sensor_bp =
            bp_library
                .find(blueprint)
                .ok_or_else(|| ActorFactoryError::SensorSpawnFailed {
                    sensor_id: blueprint.to_string(),
                    vehicle_id: format!("actor_{}", parent_id),
                    message: format!("blueprint '{}' not found", blueprint),
                })?;

        for (key, value) in attributes {
            let success = sensor_bp.set_attribute(key, value);
            if !success {
                warn!(key, value, "failed to set sensor attribute");
            }
        }

        let carla_transform =
            Self::to_carla_transform(Some(transform)).expect("sensor transform must exist");
        let actor = world
            .spawn_actor_attached(&sensor_bp, &carla_transform, parent_actor, None)
            .map_err(|e| ActorFactoryError::SensorSpawnFailed {
                sensor_id: blueprint.to_string(),
                vehicle_id: format!("actor_{}", parent_id),
                message: e.to_string(),
            })?;

        Sensor::try_from(actor).map_err(|_| ActorFactoryError::SensorSpawnFailed {
            sensor_id: blueprint.to_string(),
            vehicle_id: format!("actor_{}", parent_id),
            message: "spawned actor is not a sensor".to_string(),
        })
    }

    fn destroy_vehicle_actor(vehicle: Vehicle, actor_id: ActorId) {
        if !vehicle.destroy() {
            warn!(actor_id, "destroy vehicle returned false");
        }
    }

    fn destroy_sensor_actor(sensor: Sensor, actor_id: ActorId) {
        if sensor.is_listening() {
            sensor.stop();
        }
        if !sensor.destroy() {
            warn!(actor_id, "destroy sensor returned false");
        }
    }

    /// 转换内部 Transform 到 CARLA Transform
    fn to_carla_transform(transform: Option<Transform>) -> Option<CarlaTransform> {
        let transform = transform?;

        let location = Location {
            x: transform.location.x as f32,
            y: transform.location.y as f32,
            z: transform.location.z as f32,
        };
        let rotation = Rotation {
            pitch: transform.rotation.pitch as f32,
            yaw: transform.rotation.yaw as f32,
            roll: transform.rotation.roll as f32,
        };
        Some(CarlaTransform { location, rotation })
    }

    /// 获取底层的 CARLA Sensor 对象
    ///
    /// 用于将 Sensor 传递给 IngestionPipeline
    pub fn get_sensor(&self, actor_id: ActorId) -> Option<Sensor> {
        let actors = self.actors.lock().unwrap();
        match actors.get(&actor_id) {
            Some(ActorType::Sensor(sensor)) => Some(sensor.clone()),
            _ => None,
        }
    }
}

impl CarlaClient for RealCarlaClient {
    #[instrument(name = "real_carla_connect", skip(self), fields(host = %host, port))]
    async fn connect(&mut self, host: &str, port: u16) -> Result<()> {
        let client = Client::connect(host, port, None);
        let world = client.world();

        info!(
            map = %world.map().name(),
            "connected to CARLA server"
        );

        *self.client.lock().unwrap() = Some(client);
        *self.world.lock().unwrap() = Some(world);

        Ok(())
    }

    #[instrument(
        name = "real_carla_spawn_vehicle",
        skip(self, transform),
        fields(blueprint = %blueprint)
    )]
    async fn spawn_vehicle(
        &self,
        blueprint: &str,
        transform: Option<Transform>,
    ) -> Result<ActorId> {
        let vehicle =
            self.with_world_mut(|world| Self::create_vehicle(world, blueprint, transform))?;
        let actor_id = vehicle.id();

        vehicle.set_autopilot(true);
        info!(actor_id, "autopilot enabled for vehicle");
        debug!(actor_id, blueprint, "vehicle spawned");
        self.store_actor(actor_id, ActorType::Vehicle(vehicle));

        Ok(actor_id)
    }

    #[instrument(
        name = "real_carla_spawn_sensor",
        skip(self, transform, attributes),
        fields(blueprint = %blueprint, parent_id)
    )]
    async fn spawn_sensor(
        &self,
        blueprint: &str,
        transform: Transform,
        parent_id: ActorId,
        attributes: &HashMap<String, String>,
    ) -> Result<ActorId> {
        let parent_actor = self.parent_vehicle_for_sensor(blueprint, parent_id)?;
        let sensor = self.with_world_mut(|world| {
            Self::create_sensor(
                world,
                blueprint,
                transform,
                &parent_actor,
                parent_id,
                attributes,
            )
        })?;

        let actor_id = sensor.id();

        debug!(
            actor_id,
            blueprint, parent_id, "sensor spawned and attached"
        );
        self.store_actor(actor_id, ActorType::Sensor(sensor));

        Ok(actor_id)
    }

    #[instrument(name = "real_carla_destroy_actor", skip(self), fields(actor_id))]
    async fn destroy_actor(&self, actor_id: ActorId) -> Result<()> {
        let mut actors = self.actors.lock().unwrap();

        if let Some(actor) = actors.remove(&actor_id) {
            match actor {
                ActorType::Vehicle(v) => Self::destroy_vehicle_actor(v, actor_id),
                ActorType::Sensor(s) => Self::destroy_sensor_actor(s, actor_id),
            }
            debug!(actor_id, "actor destroyed");
        }

        // 幂等：即使不存在也返回 Ok
        Ok(())
    }

    #[instrument(name = "real_carla_actor_exists", skip(self), fields(actor_id))]
    async fn actor_exists(&self, actor_id: ActorId) -> Result<bool> {
        Ok(self.actors.lock().unwrap().contains_key(&actor_id))
    }
}

#[cfg(test)]
mod tests {
    // 真实客户端测试需要 CARLA 服务器运行
    // 这些测试被标记为 ignore，仅在有服务器时运行

    use super::*;

    #[tokio::test]
    #[ignore = "requires CARLA server"]
    async fn test_real_client_connect() {
        let mut client = RealCarlaClient::new();
        client.connect("192.168.31.193", 2000).await.unwrap();
    }
}
