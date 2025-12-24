//! CARLA Sensor SensorSource wrapper
//!
//! Wraps CARLA native Sensor as a type implementing `SensorSource` trait.
//! Only compiled when `real-carla` feature is enabled.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use carla::client::Sensor;
use contracts::{SensorDataCallback, SensorSource, SensorType};
use tracing::{debug, trace, warn};

use crate::sensor_data_converter::convert_sensor_data;

/// CARLA Sensor wrapper
///
/// Wraps CARLA native `Sensor` as `SensorSource`,
/// allowing IngestionPipeline to handle real sensors and Mock sensors uniformly.
pub struct CarlaSensorSource {
    sensor_id: String,
    sensor_type: SensorType,
    sensor: Sensor,
    listening: Arc<AtomicBool>,
}

impl CarlaSensorSource {
    /// Create new CARLA sensor source
    pub fn new(sensor_id: String, sensor_type: SensorType, sensor: Sensor) -> Self {
        Self {
            sensor_id,
            sensor_type,
            sensor,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SensorSource for CarlaSensorSource {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        self.sensor_type
    }

    fn listen(&self, callback: SensorDataCallback) {
        // Idempotent: if already listening, don't register again
        if self.listening.swap(true, Ordering::SeqCst) {
            warn!(sensor_id = %self.sensor_id, "sensor already listening");
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let sensor_type = self.sensor_type;
        let listening = self.listening.clone();

        debug!(sensor_id = %sensor_id, sensor_type = ?sensor_type, "starting CARLA sensor");

        self.sensor.listen(move |sensor_data| {
            if !listening.load(Ordering::Relaxed) {
                return;
            }

            match convert_sensor_data(&sensor_id, sensor_type, &sensor_data) {
                Some(packet) => {
                    trace!(
                        sensor_id = %sensor_id,
                        frame_id = packet.frame_id,
                        "CARLA sensor data received"
                    );
                    callback(packet);
                }
                None => {
                    trace!(sensor_id = %sensor_id, "failed to convert sensor data");
                }
            }
        });
    }

    fn stop(&self) {
        if self.listening.swap(false, Ordering::SeqCst) {
            debug!(sensor_id = %self.sensor_id, "stopping CARLA sensor");
            self.sensor.stop();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}
