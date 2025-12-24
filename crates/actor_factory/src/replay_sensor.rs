//! Replay Sensor - 从录制文件回放传感器数据
//!
//! 读取 Python 脚本录制的 JSONL + 二进制文件，
//! 按原始时间戳回放传感器数据。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use bytes::Bytes;
use contracts::{
    GnssData, ImageData, ImageFormat, ImuData, PointCloudData, RadarData, SensorDataCallback,
    SensorPacket, SensorPayload, SensorSource, SensorType, Vector3,
};
use serde::Deserialize;
use tracing::{debug, info, warn};

/// Replay 配置
#[derive(Debug, Clone, Default)]
pub struct ReplayConfig {
    /// 录制文件根目录
    pub replay_path: Option<PathBuf>,

    /// 回放速度倍率 (1.0 = 原速)
    pub speed_multiplier: f64,

    /// 是否循环回放
    pub loop_playback: bool,
}

/// 录制会话 manifest
#[derive(Debug, Deserialize)]
pub struct RecordingManifest {
    pub version: String,
    pub created_at: String,
    pub carla_version: String,
    pub duration_sec: f64,
    pub sensors: HashMap<String, SensorMetadata>,
}

/// 传感器元数据
#[derive(Debug, Deserialize)]
pub struct SensorMetadata {
    pub sensor_type: String,
    pub frame_count: u64,
}

/// JSONL 中的传感器记录
#[derive(Debug, Deserialize)]
struct SensorRecord {
    sensor_id: String,
    sensor_type: String,
    timestamp: f64,
    frame_id: u64,

    // Camera 字段
    #[serde(default)]
    data_file: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    format: Option<String>,

    // LiDAR 字段
    #[serde(default)]
    num_points: Option<u32>,
    #[serde(default)]
    point_stride: Option<u32>,

    // IMU 字段
    #[serde(default)]
    accelerometer: Option<[f64; 3]>,
    #[serde(default)]
    gyroscope: Option<[f64; 3]>,
    #[serde(default)]
    compass: Option<f64>,

    // GNSS 字段
    #[serde(default)]
    latitude: Option<f64>,
    #[serde(default)]
    longitude: Option<f64>,
    #[serde(default)]
    altitude: Option<f64>,

    // Radar 字段
    #[serde(default)]
    num_detections: Option<u32>,
}

/// Replay Sensor - 从录制文件回放传感器数据
pub struct ReplaySensor {
    sensor_id: String,
    sensor_type: SensorType,
    replay_path: PathBuf,
    records: Vec<SensorRecord>,
    config: ReplayConfig,
    listening: Arc<AtomicBool>,
    thread_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl ReplaySensor {
    /// 从录制目录加载传感器
    pub fn load(
        replay_path: &Path,
        sensor_id: String,
        sensor_type: SensorType,
        config: ReplayConfig,
    ) -> std::io::Result<Self> {
        let jsonl_path = replay_path.join("sensors.jsonl");
        let file = File::open(&jsonl_path)?;
        let reader = BufReader::new(file);

        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            let record: SensorRecord = serde_json::from_str(&line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            // 只保留该传感器的记录
            if record.sensor_id == sensor_id {
                records.push(record);
            }
        }

        // 按时间戳排序
        records.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());

        info!(
            sensor_id = %sensor_id,
            records = records.len(),
            "Loaded replay sensor"
        );

        Ok(Self {
            sensor_id,
            sensor_type,
            replay_path: replay_path.to_path_buf(),
            records,
            config,
            listening: Arc::new(AtomicBool::new(false)),
            thread_handle: std::sync::Mutex::new(None),
        })
    }

    /// 从记录构建 SensorPacket
    fn build_packet(&self, record: &SensorRecord) -> Option<SensorPacket> {
        let payload = match self.sensor_type {
            SensorType::Camera => self.build_camera_payload(record)?,
            SensorType::Lidar => self.build_lidar_payload(record)?,
            SensorType::Imu => self.build_imu_payload(record)?,
            SensorType::Gnss => self.build_gnss_payload(record)?,
            SensorType::Radar => self.build_radar_payload(record)?,
        };

        Some(SensorPacket {
            sensor_id: self.sensor_id.clone().into(),
            sensor_type: self.sensor_type,
            timestamp: record.timestamp,
            frame_id: Some(record.frame_id),
            payload,
        })
    }

    fn build_camera_payload(&self, record: &SensorRecord) -> Option<SensorPayload> {
        let data_file = record.data_file.as_ref()?;
        let data = self.read_binary_file(data_file)?;

        Some(SensorPayload::Image(ImageData {
            width: record.width.unwrap_or(0),
            height: record.height.unwrap_or(0),
            format: ImageFormat::Bgra8,
            data,
        }))
    }

    fn build_lidar_payload(&self, record: &SensorRecord) -> Option<SensorPayload> {
        let data_file = record.data_file.as_ref()?;
        let data = self.read_binary_file(data_file)?;

        Some(SensorPayload::PointCloud(PointCloudData {
            num_points: record.num_points.unwrap_or(0),
            point_stride: record.point_stride.unwrap_or(16),
            data,
        }))
    }

    fn build_imu_payload(&self, record: &SensorRecord) -> Option<SensorPayload> {
        let accel = record.accelerometer?;
        let gyro = record.gyroscope?;

        Some(SensorPayload::Imu(ImuData {
            accelerometer: Vector3 {
                x: accel[0],
                y: accel[1],
                z: accel[2],
            },
            gyroscope: Vector3 {
                x: gyro[0],
                y: gyro[1],
                z: gyro[2],
            },
            compass: record.compass.unwrap_or(0.0),
        }))
    }

    fn build_gnss_payload(&self, record: &SensorRecord) -> Option<SensorPayload> {
        Some(SensorPayload::Gnss(GnssData {
            latitude: record.latitude?,
            longitude: record.longitude?,
            altitude: record.altitude?,
        }))
    }

    fn build_radar_payload(&self, record: &SensorRecord) -> Option<SensorPayload> {
        let data_file = record.data_file.as_ref()?;
        let data = self.read_binary_file(data_file)?;

        Some(SensorPayload::Radar(RadarData {
            num_detections: record.num_detections.unwrap_or(0),
            data,
        }))
    }

    fn read_binary_file(&self, relative_path: &str) -> Option<Bytes> {
        let path = self.replay_path.join(relative_path);
        match std::fs::read(&path) {
            Ok(data) => Some(Bytes::from(data)),
            Err(e) => {
                warn!(path = %path.display(), error = %e, "Failed to read binary file");
                None
            }
        }
    }
}

impl SensorSource for ReplaySensor {
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

        let listening = self.listening.clone();
        let sensor_id = self.sensor_id.clone();
        let records = self.records.clone();
        let replay_path = self.replay_path.clone();
        let sensor_type = self.sensor_type;
        let speed = self.config.speed_multiplier.max(0.1);
        let loop_playback = self.config.loop_playback;

        let handle = thread::spawn(move || {
            debug!(sensor_id = %sensor_id, "Replay thread started");

            loop {
                if records.is_empty() {
                    warn!(sensor_id = %sensor_id, "No records to replay");
                    break;
                }

                let start_time = Instant::now();
                let first_timestamp = records[0].timestamp;

                for record in &records {
                    if !listening.load(Ordering::Relaxed) {
                        debug!(sensor_id = %sensor_id, "Replay stopped");
                        return;
                    }

                    // 计算等待时间
                    let record_offset = record.timestamp - first_timestamp;
                    let target_elapsed = Duration::from_secs_f64(record_offset / speed);
                    let actual_elapsed = start_time.elapsed();

                    if target_elapsed > actual_elapsed {
                        thread::sleep(target_elapsed - actual_elapsed);
                    }

                    // 构建并发送 packet
                    let replay_sensor = ReplaySensor {
                        sensor_id: sensor_id.clone(),
                        sensor_type,
                        replay_path: replay_path.clone(),
                        records: vec![],
                        config: ReplayConfig::default(),
                        listening: Arc::new(AtomicBool::new(false)),
                        thread_handle: std::sync::Mutex::new(None),
                    };

                    if let Some(packet) = replay_sensor.build_packet(record) {
                        callback(packet);
                    }
                }

                if !loop_playback {
                    info!(sensor_id = %sensor_id, "Replay completed");
                    break;
                }

                debug!(sensor_id = %sensor_id, "Looping replay");
            }

            listening.store(false, Ordering::SeqCst);
        });

        *self.thread_handle.lock().unwrap() = Some(handle);
    }

    fn stop(&self) {
        self.listening.store(false, Ordering::SeqCst);

        // 等待线程结束
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}

// 让 SensorRecord Clone 以便在线程中使用
impl Clone for SensorRecord {
    fn clone(&self) -> Self {
        Self {
            sensor_id: self.sensor_id.clone(),
            sensor_type: self.sensor_type.clone(),
            timestamp: self.timestamp,
            frame_id: self.frame_id,
            data_file: self.data_file.clone(),
            width: self.width,
            height: self.height,
            format: self.format.clone(),
            num_points: self.num_points,
            point_stride: self.point_stride,
            accelerometer: self.accelerometer,
            gyroscope: self.gyroscope,
            compass: self.compass,
            latitude: self.latitude,
            longitude: self.longitude,
            altitude: self.altitude,
            num_detections: self.num_detections,
        }
    }
}
