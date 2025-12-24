//! Sensor adapter macros
//!
//! Use declarative macros to eliminate repetitive code templates in adapters

/// Define common template for sensor adapters
///
/// This macro generates all boilerplate code shared by adapters, including:
/// - Structure definition
/// - `new` and `new_mock` constructors
/// - `SensorAdapter` trait implementation
/// - Backpressure handling and metrics recording
///
/// # Usage
/// ```ignore
/// define_sensor_adapter!(
///     CameraAdapter,           // Adapter name
///     SensorType::Camera,      // Sensor type
///     Image,                   // CARLA data type
///     image_to_payload         // Payload conversion function
/// );
/// ```
macro_rules! define_sensor_adapter {
    (
        $adapter_name:ident,
        $sensor_type:expr,
        $carla_type:ident,
        $to_payload_fn:ident
    ) => {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        use contracts::{SensorPacket, SensorType};
        use async_channel::Sender;
        #[cfg(feature = "real-carla")]
        use tracing::{debug, trace, warn};
        #[cfg(not(feature = "real-carla"))]
        use tracing::{debug, warn};

        #[cfg(feature = "real-carla")]
        use carla::client::Sensor;
        #[cfg(feature = "real-carla")]
        use carla::sensor::SensorDataBase;

        use crate::adapter::SensorAdapter;
        #[cfg(feature = "real-carla")]
        use crate::adapters::common::send_packet;
        use crate::config::{BackpressureConfig, IngestionMetrics};

        #[allow(dead_code)] // config field used only with real-carla
        pub struct $adapter_name {
            sensor_id: String,
            config: BackpressureConfig,
            listening: Arc<AtomicBool>,
            #[cfg(feature = "real-carla")]
            sensor: Sensor,
        }

        impl $adapter_name {
            #[cfg(feature = "real-carla")]
            pub fn new(sensor_id: String, sensor: Sensor, config: BackpressureConfig) -> Self {
                Self {
                    sensor_id,
                    config,
                    listening: Arc::new(AtomicBool::new(false)),
                    sensor,
                }
            }

            #[cfg(not(feature = "real-carla"))]
            pub fn new_mock(sensor_id: String, config: BackpressureConfig) -> Self {
                Self {
                    sensor_id,
                    config,
                    listening: Arc::new(AtomicBool::new(false)),
                }
            }
        }

        impl SensorAdapter for $adapter_name {
            fn sensor_id(&self) -> &str {
                &self.sensor_id
            }

            fn sensor_type(&self) -> SensorType {
                $sensor_type
            }

            #[cfg(feature = "real-carla")]
            fn start(&self, tx: Sender<SensorPacket>, metrics: Arc<IngestionMetrics>) {
                if self.listening.swap(true, Ordering::SeqCst) {
                    warn!(sensor_id = %self.sensor_id, "adapter already listening");
                    return;
                }

                let sensor_id = self.sensor_id.clone();
                let sensor_id_arc: contracts::SensorId = sensor_id.clone().into();
                let drop_policy = self.config.drop_policy;
                let listening = self.listening.clone();

                debug!(sensor_id = %sensor_id, sensor_type = ?$sensor_type, "starting adapter");

                self.sensor.listen(move |sensor_data| {
                    if !listening.load(Ordering::Relaxed) {
                        return;
                    }

                    let data = match $carla_type::try_from(sensor_data.clone()) {
                        Ok(d) => d,
                        Err(_) => {
                            metrics.record_parse_error();
                            trace!(sensor_id = %sensor_id, "failed to parse sensor data");
                            return;
                        }
                    };

                    let packet = SensorPacket {
                        sensor_id: sensor_id_arc.clone(),
                        sensor_type: $sensor_type,
                        timestamp: sensor_data.timestamp(),
                        frame_id: Some(sensor_data.frame() as u64),
                        payload: $to_payload_fn(&data),
                    };

                    metrics.record_received();
                    send_packet(&tx, packet, &metrics, &sensor_id, drop_policy);
                });
            }

            #[cfg(not(feature = "real-carla"))]
            fn start(&self, _tx: Sender<SensorPacket>, _metrics: Arc<IngestionMetrics>) {
                self.listening.store(true, Ordering::SeqCst);
                warn!(sensor_id = %self.sensor_id, "adapter started in mock mode");
            }

            fn stop(&self) {
                if self.listening.swap(false, Ordering::SeqCst) {
                    debug!(sensor_id = %self.sensor_id, "stopping adapter");
                    #[cfg(feature = "real-carla")]
                    self.sensor.stop();
                }
            }

            fn is_listening(&self) -> bool {
                self.listening.load(Ordering::Relaxed)
            }
        }
    };
}
