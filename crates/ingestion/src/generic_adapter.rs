//! Generic sensor adapter
//!
//! Unified adapter implementation based on `SensorSource` trait.
//! Allows IngestionPipeline to handle Mock and Real sensors uniformly.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_channel::Sender;
use contracts::{SensorDataCallback, SensorPacket, SensorSource, SensorType};
use tracing::{debug, trace};

use crate::adapter::SensorAdapter;
use crate::adapters::common::send_packet;
use crate::config::{BackpressureConfig, IngestionMetrics};

/// Generic sensor adapter
///
/// Adapts `SensorSource` trait to `SensorAdapter`.
/// This is the bridge connecting actor_factory and ingestion.
pub struct GenericSensorAdapter {
    sensor_id: String,
    source: Box<dyn SensorSource>,
    config: BackpressureConfig,
    listening: Arc<AtomicBool>,
}

impl GenericSensorAdapter {
    /// Create new generic adapter
    pub fn new(
        sensor_id: String,
        source: Box<dyn SensorSource>,
        config: BackpressureConfig,
    ) -> Self {
        Self {
            sensor_id,
            source,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SensorAdapter for GenericSensorAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        self.source.sensor_type()
    }

    fn start(&self, tx: Sender<SensorPacket>, metrics: Arc<IngestionMetrics>) {
        if self.listening.swap(true, Ordering::SeqCst) {
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let drop_policy = self.config.drop_policy;
        let listening = self.listening.clone();

        debug!(sensor_id = %sensor_id, "starting generic adapter");

        let callback: SensorDataCallback = Arc::new(move |packet| {
            if !listening.load(Ordering::Relaxed) {
                return;
            }

            metrics.record_received();
            trace!(sensor_id = %sensor_id, "generic adapter received packet");
            send_packet(&tx, packet, &metrics, &sensor_id, drop_policy);
        });

        self.source.listen(callback);
    }

    fn stop(&self) {
        if self.listening.swap(false, Ordering::SeqCst) {
            debug!(sensor_id = %self.sensor_id, "stopping generic adapter");
            self.source.stop();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DropPolicy;
    use async_channel::bounded;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    /// Mock SensorSource for testing
    struct TestSensorSource {
        sensor_id: String,
        sensor_type: SensorType,
        listening: Arc<AtomicBool>,
    }

    impl TestSensorSource {
        fn new(sensor_id: &str, sensor_type: SensorType) -> Self {
            Self {
                sensor_id: sensor_id.to_string(),
                sensor_type,
                listening: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    impl SensorSource for TestSensorSource {
        fn sensor_id(&self) -> &str {
            &self.sensor_id
        }

        fn sensor_type(&self) -> SensorType {
            self.sensor_type
        }

        fn listen(&self, callback: SensorDataCallback) {
            if self.listening.swap(true, Ordering::SeqCst) {
                return;
            }

            let sensor_id = self.sensor_id.clone();
            let sensor_type = self.sensor_type;
            let listening = self.listening.clone();

            std::thread::spawn(move || {
                let mut frame_id = 0u64;
                while listening.load(Ordering::Relaxed) {
                    frame_id += 1;
                    let packet = SensorPacket {
                        sensor_id: sensor_id.clone().into(),
                        sensor_type,
                        timestamp: frame_id as f64 * 0.033,
                        frame_id: Some(frame_id),
                        payload: contracts::SensorPayload::Imu(contracts::ImuData {
                            accelerometer: contracts::Vector3::default(),
                            gyroscope: contracts::Vector3::default(),
                            compass: 0.0,
                        }),
                    };
                    callback(packet);
                    std::thread::sleep(Duration::from_millis(33));
                }
            });
        }

        fn stop(&self) {
            self.listening.store(false, Ordering::SeqCst);
        }

        fn is_listening(&self) -> bool {
            self.listening.load(Ordering::Relaxed)
        }
    }

    #[test]
    fn test_generic_adapter() {
        let source = TestSensorSource::new("test", SensorType::Imu);
        let adapter = GenericSensorAdapter::new(
            "test".to_string(),
            Box::new(source),
            BackpressureConfig {
                channel_capacity: 10,
                drop_policy: DropPolicy::DropNewest,
            },
        );

        let (tx, rx) = bounded(10);
        let metrics = Arc::new(IngestionMetrics::new());

        adapter.start(tx, metrics.clone());
        assert!(adapter.is_listening());

        // Wait for some packets
        std::thread::sleep(Duration::from_millis(100));

        adapter.stop();
        assert!(!adapter.is_listening());

        // Should have received some packets
        let count = Arc::new(AtomicU64::new(0));
        while rx.try_recv().is_ok() {
            count.fetch_add(1, Ordering::Relaxed);
        }
        assert!(count.load(Ordering::Relaxed) > 0);
    }
}
