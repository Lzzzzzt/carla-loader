//! # Integration Tests
//!
//! Integration tests and end-to-end tests.
//!
//! Responsibilities:
//! - Contract snapshot tests
//! - Simulated e2e tests (no CARLA required)
//! - Performance regression baselines

#[cfg(test)]
mod contract_tests {
    #[test]
    fn test_contracts_compile() {
        // Verify contracts crate can compile
        let _ = contracts::ConfigVersion::V1;
    }
}

#[cfg(test)]
mod e2e_tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use contracts::{SinkConfig, SinkType, SyncedFrame};
    use dispatcher::create_dispatcher;
    use ingestion::MockSensorSource;
    use sync_engine::{MissingDataStrategy, SyncEngine, SyncEngineConfig};
    use tokio::sync::mpsc;

    /// End-to-end test: MockSensorSource -> SyncEngine -> Dispatcher
    ///
    /// Verifies complete data flow:
    /// 1. MockSensorSource generates sensor data
    /// 2. SyncEngine synchronizes multi-sensor frames
    /// 3. Dispatcher distributes SyncedFrame to sinks
    #[tokio::test]
    async fn test_e2e_mock_pipeline() {
        // Setup: Create mock sensor sources
        let camera_source = MockSensorSource::camera("cam", 20.0, 100, 100);
        let lidar_source = MockSensorSource::lidar("lidar", 10.0, 1000);

        // Create sync engine
        let sync_config = SyncEngineConfig {
            reference_sensor_id: "cam".into(),
            required_sensors: vec!["cam".into(), "lidar".into()],
            imu_sensor_id: None,
            window: Default::default(),
            buffer: Default::default(),
            adakf: Default::default(),
            missing_strategy: MissingDataStrategy::Drop,
            sensor_intervals: HashMap::new(),
        };
        let mut sync_engine = SyncEngine::new(sync_config);

        // Create dispatcher with log sink
        let (sync_tx, sync_rx) = mpsc::channel::<SyncedFrame>(100);
        let sink_configs = vec![SinkConfig {
            name: "test_log".to_string(),
            sink_type: SinkType::Log,
            queue_capacity: 50,
            params: HashMap::new(),
        }];

        let dispatcher = create_dispatcher(sink_configs, sync_rx).await.unwrap();
        let dispatcher_handle = dispatcher.spawn();

        // Start mock sources (async-channel receivers)
        let camera_rx = camera_source.start(100, None);
        let lidar_rx = lidar_source.start(100, None);

        // Counter for verification
        let frame_count = Arc::new(AtomicU64::new(0));
        let target_frames = 5u64;

        // Fan-in async channels to tokio mpsc
        let (bridge_tx, mut bridge_rx) = mpsc::channel(200);
        let bridge_tx_cam = bridge_tx.clone();
        let bridge_tx_lidar = bridge_tx.clone();
        drop(bridge_tx);

        // async-channel is natively async
        tokio::spawn(async move {
            while let Ok(packet) = camera_rx.recv().await {
                if bridge_tx_cam.send(packet).await.is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            while let Ok(packet) = lidar_rx.recv().await {
                if bridge_tx_lidar.send(packet).await.is_err() {
                    break;
                }
            }
        });

        // Run pipeline
        let sync_tx_clone = sync_tx.clone();
        let frame_count_clone = frame_count.clone();

        let pipeline_handle = tokio::spawn(async move {
            let mut cam_received = false;
            let mut lidar_received = false;

            while let Some(packet) = bridge_rx.recv().await {
                if packet.sensor_id == "cam" {
                    cam_received = true;
                } else if packet.sensor_id == "lidar" {
                    lidar_received = true;
                }

                if let Some(frame) = sync_engine.push(packet) {
                    frame_count_clone.fetch_add(1, Ordering::SeqCst);
                    if sync_tx_clone.send(frame).await.is_err() {
                        break;
                    }
                    if frame_count_clone.load(Ordering::SeqCst) >= target_frames {
                        break;
                    }
                }
            }

            // Return stats
            (cam_received, lidar_received, sync_engine.frame_count())
        });

        // Wait for pipeline with timeout
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), pipeline_handle).await;

        // Stop sources
        camera_source.stop();
        lidar_source.stop();

        // Close sync channel to shutdown dispatcher
        drop(sync_tx);

        // Wait for dispatcher
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), dispatcher_handle).await;

        // Verify results
        assert!(result.is_ok(), "Pipeline timed out");
        let (cam_received, lidar_received, engine_frame_count) = result.unwrap().unwrap();
        assert!(cam_received, "Camera packets should be received");
        assert!(lidar_received, "LiDAR packets should be received");
        assert!(
            engine_frame_count >= target_frames,
            "Should produce at least {} synced frames, got {}",
            target_frames,
            engine_frame_count
        );
    }

    /// Test SyncEngine with IMU for adaptive window
    #[tokio::test]
    async fn test_sync_engine_with_imu() {
        let camera_source = MockSensorSource::camera("cam", 30.0, 100, 100);
        let lidar_source = MockSensorSource::lidar("lidar", 10.0, 1000);
        let imu_source = MockSensorSource::imu("imu", 100.0);

        let sync_config = SyncEngineConfig {
            reference_sensor_id: "cam".into(),
            required_sensors: vec!["cam".into(), "lidar".into()],
            imu_sensor_id: Some("imu".into()),
            window: Default::default(),
            buffer: Default::default(),
            adakf: Default::default(),
            missing_strategy: MissingDataStrategy::Drop,
            sensor_intervals: HashMap::new(),
        };
        let mut sync_engine = SyncEngine::new(sync_config);

        let camera_rx = camera_source.start(100, None);
        let lidar_rx = lidar_source.start(100, None);
        let imu_rx = imu_source.start(100, None);

        let target = 3u64;

        // Fan-in async channels to tokio mpsc
        let (bridge_tx, mut bridge_rx) = mpsc::channel(300);
        let bridge_tx_cam = bridge_tx.clone();
        let bridge_tx_lidar = bridge_tx.clone();
        let bridge_tx_imu = bridge_tx.clone();
        drop(bridge_tx);

        tokio::spawn(async move {
            while let Ok(packet) = camera_rx.recv().await {
                if bridge_tx_cam.send(packet).await.is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            while let Ok(packet) = lidar_rx.recv().await {
                if bridge_tx_lidar.send(packet).await.is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            while let Ok(packet) = imu_rx.recv().await {
                if bridge_tx_imu.send(packet).await.is_err() {
                    break;
                }
            }
        });

        let handle = tokio::spawn(async move {
            let mut synced_count = 0u64;
            while let Some(p) = bridge_rx.recv().await {
                if let Some(_frame) = sync_engine.push(p) {
                    synced_count += 1;
                    if synced_count >= target {
                        break;
                    }
                }
            }
            synced_count
        });

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;

        camera_source.stop();
        lidar_source.stop();
        imu_source.stop();

        assert!(result.is_ok(), "Test timed out");
        let count = result.unwrap().unwrap();
        assert!(
            count >= target,
            "Should produce at least {} frames, got {}",
            target,
            count
        );
    }

    /// Test dispatcher with multiple sink types
    #[tokio::test]
    async fn test_dispatcher_multiple_sinks() {
        let (tx, rx) = mpsc::channel::<SyncedFrame>(10);

        // Create multiple sinks
        let sink_configs = vec![
            SinkConfig {
                name: "log1".to_string(),
                sink_type: SinkType::Log,
                queue_capacity: 50,
                params: HashMap::new(),
            },
            SinkConfig {
                name: "log2".to_string(),
                sink_type: SinkType::Log,
                queue_capacity: 50,
                params: HashMap::new(),
            },
        ];

        let dispatcher = create_dispatcher(sink_configs, rx).await.unwrap();

        // Check metrics before running
        let metrics = dispatcher.metrics();
        assert_eq!(metrics.len(), 2);

        let handle = dispatcher.spawn();

        // Send frames
        for i in 0..5 {
            let frame = SyncedFrame {
                t_sync: i as f64 * 0.1,
                frame_id: i,
                frames: HashMap::new(),
                sync_meta: contracts::SyncMeta::default(),
            };
            tx.send(frame).await.unwrap();
        }

        // Close channel
        drop(tx);

        // Wait for dispatcher
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
    }
}
