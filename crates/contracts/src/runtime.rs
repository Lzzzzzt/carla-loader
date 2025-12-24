//! RuntimeGraph - Actor Factory output
//!
//! Runtime actor handles and mappings.

use std::collections::HashMap;

/// CARLA actor handle type
pub type ActorId = u32;

/// Runtime actor graph
///
/// Contains all created CARLA actors and their mappings.
#[derive(Debug, Clone)]
pub struct RuntimeGraph {
    /// Vehicle ID -> Actor handle
    pub vehicles: HashMap<String, ActorId>,

    /// Sensor ID -> Actor handle
    pub sensors: HashMap<String, ActorId>,

    /// Sensor ID -> Parent vehicle ID
    pub sensor_to_vehicle: HashMap<String, String>,

    /// Actor handle -> Config ID (reverse lookup)
    pub actor_to_id: HashMap<ActorId, String>,
}

impl RuntimeGraph {
    /// Create empty RuntimeGraph
    pub fn new() -> Self {
        Self {
            vehicles: HashMap::new(),
            sensors: HashMap::new(),
            sensor_to_vehicle: HashMap::new(),
            actor_to_id: HashMap::new(),
        }
    }

    /// Register vehicle
    pub fn register_vehicle(&mut self, id: String, actor_id: ActorId) {
        self.actor_to_id.insert(actor_id, id.clone());
        self.vehicles.insert(id, actor_id);
    }

    /// Register sensor
    pub fn register_sensor(&mut self, sensor_id: String, vehicle_id: String, actor_id: ActorId) {
        self.actor_to_id.insert(actor_id, sensor_id.clone());
        self.sensor_to_vehicle.insert(sensor_id.clone(), vehicle_id);
        self.sensors.insert(sensor_id, actor_id);
    }

    /// Get all actor handles (for teardown)
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
