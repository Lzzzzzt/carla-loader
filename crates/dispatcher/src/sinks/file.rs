//! FileSink - writes frames to disk with folder structure

use contracts::{
    ContractError, DataSink, ImageData, ImageFormat, PointCloudData, SensorPayload, SyncedFrame,
};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, error, instrument};

/// Configuration for FileSink
#[derive(Debug, Clone)]
pub struct FileSinkConfig {
    /// Base output directory
    pub base_path: PathBuf,
}

impl FileSinkConfig {
    /// Create config from params map
    pub fn from_params(params: &HashMap<String, String>) -> Self {
        let base_path = params
            .get("base_path")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./output"));

        Self { base_path }
    }
}

/// Sink that writes frames to disk files
pub struct FileSink {
    name: String,
    config: FileSinkConfig,
    created_dirs: HashSet<PathBuf>,
}

impl FileSink {
    /// Create a new FileSink
    pub fn new(name: impl Into<String>, config: FileSinkConfig) -> std::io::Result<Self> {
        // Create base directory if it doesn't exist
        fs::create_dir_all(&config.base_path)?;

        Ok(Self {
            name: name.into(),
            config,
            created_dirs: HashSet::new(),
        })
    }

    /// Create from params map (for factory)
    pub fn from_params(
        name: impl Into<String>,
        params: &HashMap<String, String>,
    ) -> std::io::Result<Self> {
        let config = FileSinkConfig::from_params(params);
        Self::new(name, config)
    }

    fn write_frame_to_disk(&mut self, frame: &SyncedFrame) -> std::io::Result<()> {
        let frame_id = frame.frame_id;

        // 1. Write SyncMeta
        let meta_dir = self.config.base_path.join("meta");
        if !self.created_dirs.contains(&meta_dir) {
            fs::create_dir_all(&meta_dir)?;
            self.created_dirs.insert(meta_dir.clone());
        }
        let meta_path = meta_dir.join(format!("{}.json", frame_id));
        let meta_file = File::create(meta_path)?;
        serde_json::to_writer(meta_file, &frame.sync_meta)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // 2. Write Sensor Packets
        for (sensor_id, packet) in &frame.frames {
            self.write_sensor_data(sensor_id, frame_id, &packet.payload)?;
        }

        Ok(())
    }

    fn write_sensor_data(
        &mut self,
        sensor_id: &str,
        frame_id: u64,
        payload: &SensorPayload,
    ) -> std::io::Result<()> {
        let sensor_dir = self.config.base_path.join(sensor_id);
        if !self.created_dirs.contains(&sensor_dir) {
            fs::create_dir_all(&sensor_dir)?;
            self.created_dirs.insert(sensor_dir.clone());
        }

        match payload {
            SensorPayload::Image(image_data) => {
                let filename = format!("{}.png", frame_id);
                let path = sensor_dir.join(filename);
                self.save_image(path, image_data)?;
            }
            SensorPayload::PointCloud(pc_data) => {
                let filename = format!("{}.ply", frame_id);
                let path = sensor_dir.join(filename);
                self.save_point_cloud(path, pc_data)?;
            }
            _ => {
                // Fallback to JSON for other types
                let filename = format!("{}.json", frame_id);
                let path = sensor_dir.join(filename);
                let file = File::create(path)?;
                serde_json::to_writer(file, payload)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            }
        }
        Ok(())
    }

    fn save_image(&self, path: PathBuf, image: &ImageData) -> std::io::Result<()> {
        match image.format {
            ImageFormat::Rgb8 => image::save_buffer(
                path,
                &image.data,
                image.width,
                image.height,
                image::ColorType::Rgb8,
            )
            .map_err(std::io::Error::other),

            ImageFormat::Rgba8 => image::save_buffer(
                path,
                &image.data,
                image.width,
                image.height,
                image::ColorType::Rgba8,
            )
            .map_err(std::io::Error::other),

            ImageFormat::Bgra8 => {
                // Convert BGRA to RGBA
                let mut rgba_data = image.data.to_vec();
                for chunk in rgba_data.chunks_exact_mut(4) {
                    chunk.swap(0, 2); // Swap B and R
                }
                image::save_buffer(
                    path,
                    &rgba_data,
                    image.width,
                    image.height,
                    image::ColorType::Rgba8,
                )
                .map_err(std::io::Error::other)
            }

            ImageFormat::Depth | ImageFormat::SemanticSeg => {
                // Save as is (usually BGRA in CARLA)
                image::save_buffer(
                    path,
                    &image.data,
                    image.width,
                    image.height,
                    image::ColorType::Rgba8,
                )
                .map_err(std::io::Error::other)
            }
        }
    }

    fn save_point_cloud(&self, path: PathBuf, pc: &PointCloudData) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        // Write PLY header
        writeln!(file, "ply")?;
        writeln!(file, "format binary_little_endian 1.0")?;
        writeln!(file, "element vertex {}", pc.num_points)?;
        writeln!(file, "property float x")?;
        writeln!(file, "property float y")?;
        writeln!(file, "property float z")?;
        // Assuming stride 16 (4 floats), 4th is usually intensity or padding.
        if pc.point_stride >= 16 {
            writeln!(file, "property float intensity")?;
        }
        writeln!(file, "end_header")?;

        // Write binary data
        file.write_all(&pc.data)?;
        Ok(())
    }

    fn persist_frame(&mut self, frame: &SyncedFrame) -> Result<(), ContractError> {
        self.write_frame_to_disk(frame).map_err(|e| {
            error!(sink = %self.name, frame_id = frame.frame_id, error = %e, "Write failed");
            ContractError::sink_write(&self.name, e.to_string())
        })
    }
}

impl DataSink for FileSink {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(
        name = "file_sink_write",
        skip(self, frame),
        fields(sink = %self.name, frame_id = frame.frame_id)
    )]
    async fn write(&mut self, frame: &SyncedFrame) -> Result<(), ContractError> {
        self.persist_frame(frame)?;
        Ok(())
    }

    #[instrument(name = "file_sink_flush", skip(self))]
    async fn flush(&mut self) -> Result<(), ContractError> {
        Ok(())
    }

    #[instrument(name = "file_sink_close", skip(self))]
    async fn close(&mut self) -> Result<(), ContractError> {
        debug!(sink = %self.name, "FileSink closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::SyncMeta;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_sink_write() {
        let dir = tempdir().unwrap();
        let config = FileSinkConfig {
            base_path: dir.path().to_path_buf(),
        };

        let mut sink = FileSink::new("test_file", config).unwrap();
        let frame = SyncedFrame {
            t_sync: 1.0,
            frame_id: 1,
            frames: HashMap::new(),
            sync_meta: SyncMeta::default(),
        };

        sink.write(&frame).await.unwrap();
        sink.flush().await.unwrap();

        // Verify meta file was created
        let meta_dir = dir.path().join("meta");
        assert!(meta_dir.exists());
        let entries: Vec<_> = fs::read_dir(meta_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }
}
