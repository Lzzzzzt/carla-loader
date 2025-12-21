//! # Integration Tests
//!
//! 集成测试与端到端测试。
//!
//! 负责：
//! - 合约快照测试
//! - 模拟 e2e 测试（无需 CARLA）
//! - 性能回归基线

#[cfg(test)]
mod contract_tests {
    #[test]
    fn test_contracts_compile() {
        // 验证 contracts crate 可编译
        let _ = contracts::ConfigVersion::V1;
    }
}

#[cfg(test)]
mod e2e_tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use contracts::{SensorPacket, SinkConfig, SinkType, SyncedFrame};
    use dispatcher::create_dispatcher;
    use ingestion::MockSensorSource;
    use sync_engine::{MissingDataStrategy, SyncEngine, SyncEngineConfig};
    use tokio::sync::mpsc;

    /// End-to-end test: MockSensorSource -> SyncEngine -> Dispatcher
    ///
    /// 验证完整的数据流：
    /// 1. MockSensorSource 生成传感器数据
    /// 2. SyncEngine 同步多传感器帧
    /// 3. Dispatcher 将 SyncedFrame 分发到 sinks
    #[tokio::test]
    async fn test_e2e_mock_pipeline() {
        // Setup: Create mock sensor sources
        let camera_source = MockSensorSource::camera("cam", 20.0, 100, 100);
        let lidar_source = MockSensorSource::lidar("lidar", 10.0, 1000);

        // Create sync engine
        let sync_config = SyncEngineConfig {
            reference_sensor_id: "cam".to_string(),
            required_sensors: vec!["cam".to_string(), "lidar".to_string()],
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

        // Start mock sources
        let mut camera_rx = camera_source.start(100, None);
        let mut lidar_rx = lidar_source.start(100, None);

        // Counter for verification
        let frame_count = Arc::new(AtomicU64::new(0));
        let target_frames = 5u64;

        // Run pipeline
        let sync_tx_clone = sync_tx.clone();
        let frame_count_clone = frame_count.clone();

        let pipeline_handle = tokio::spawn(async move {
            let mut cam_packet: Option<SensorPacket> = None;
            let mut lidar_packet: Option<SensorPacket> = None;

            loop {
                tokio::select! {
                    Some(packet) = camera_rx.recv() => {
                        cam_packet = Some(packet.clone());
                        // Try to sync
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
                    Some(packet) = lidar_rx.recv() => {
                        lidar_packet = Some(packet.clone());
                        // Try to sync
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
                    else => break,
                }
            }

            // Return stats
            (
                cam_packet.is_some(),
                lidar_packet.is_some(),
                sync_engine.frame_count(),
            )
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
            reference_sensor_id: "cam".to_string(),
            required_sensors: vec!["cam".to_string(), "lidar".to_string()],
            imu_sensor_id: Some("imu".to_string()),
            window: Default::default(),
            buffer: Default::default(),
            adakf: Default::default(),
            missing_strategy: MissingDataStrategy::Drop,
            sensor_intervals: HashMap::new(),
        };
        let mut sync_engine = SyncEngine::new(sync_config);

        let mut camera_rx = camera_source.start(100, None);
        let mut lidar_rx = lidar_source.start(100, None);
        let mut imu_rx = imu_source.start(100, None);

        let mut synced_count = 0u64;
        let target = 3u64;

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(p) = camera_rx.recv() => {
                        if let Some(_frame) = sync_engine.push(p) {
                            synced_count += 1;
                            if synced_count >= target { break; }
                        }
                    }
                    Some(p) = lidar_rx.recv() => {
                        if let Some(_frame) = sync_engine.push(p) {
                            synced_count += 1;
                            if synced_count >= target { break; }
                        }
                    }
                    Some(p) = imu_rx.recv() => {
                        // IMU updates motion intensity but doesn't produce frames
                        let _ = sync_engine.push(p);
                    }
                    else => break,
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
