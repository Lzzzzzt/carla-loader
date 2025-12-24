//! Real CARLA client implementation
//!
//! Connects to CARLA server using carla-rust crate.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use carla::client::{ActorBase, Client, Sensor, Vehicle, World};
use carla::geom::{Location, Rotation, Transform as CarlaTransform};
use contracts::{ActorId, SensorSource, SensorType, Transform};
use tracing::{debug, info, instrument, warn};

use crate::carla_sensor_source::CarlaSensorSource;
use crate::client::CarlaClient;
use crate::error::{ActorFactoryError, Result};

/// Real CARLA client
///
/// Wraps carla-rust's Client, implements CarlaClient trait.
/// Uses Mutex for interior mutability, allowing `&self` methods to modify World.
#[derive(Default, Clone)]
pub struct RealCarlaClient {
    /// CARLA client
    client: Arc<Mutex<Option<Client>>>,
    /// World reference (uses Mutex for interior mutability)
    world: Arc<Mutex<Option<World>>>,
    /// Created actors list (for teardown)
    actors: Arc<Mutex<HashMap<ActorId, ActorType>>>,
}

/// Actor type enumeration
#[derive(Clone)]
enum ActorType {
    Vehicle(Vehicle),
    Sensor(Sensor),
}

impl RealCarlaClient {
    /// Create new client (disconnected state)
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            world: Arc::new(Mutex::new(None)),
            actors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Access World with mutable reference, ensuring connected
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

    /// Save actor to registry for teardown
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

    /// Convert internal Transform to CARLA Transform
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

    /// Get underlying CARLA Sensor object
    ///
    /// Used to pass Sensor to IngestionPipeline
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

        // Idempotent: return Ok even if not exists
        Ok(())
    }

    #[instrument(name = "real_carla_actor_exists", skip(self), fields(actor_id))]
    async fn actor_exists(&self, actor_id: ActorId) -> Result<bool> {
        Ok(self.actors.lock().unwrap().contains_key(&actor_id))
    }

    fn get_sensor_source(
        &self,
        actor_id: ActorId,
        sensor_id: String,
        sensor_type: SensorType,
    ) -> Option<Box<dyn SensorSource>> {
        let sensor = self.get_sensor(actor_id)?;
        Some(Box::new(CarlaSensorSource::new(
            sensor_id,
            sensor_type,
            sensor,
        )))
    }
}

#[cfg(test)]
mod tests {
    // Real client tests require CARLA server running
    // These tests are marked as ignore, only run when server is available

    use super::*;

    #[tokio::test]
    #[ignore = "requires CARLA server"]
    async fn test_real_client_connect() {
        let mut client = RealCarlaClient::new();
        client.connect("192.168.31.193", 2000).await.unwrap();
    }
}
