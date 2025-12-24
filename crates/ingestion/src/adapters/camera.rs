//! Camera sensor adapter

#[cfg(feature = "real-carla")]
use bytes::Bytes;
#[cfg(feature = "real-carla")]
use contracts::{ImageData, ImageFormat, SensorPayload};

#[cfg(feature = "real-carla")]
use carla::sensor::data::Image;

/// Convert CARLA Image to SensorPayload
#[cfg(feature = "real-carla")]
#[inline]
fn image_to_payload(image: &Image) -> SensorPayload {
    let data = Bytes::copy_from_slice(image.as_raw_bytes());
    SensorPayload::Image(ImageData {
        width: image.width() as u32,
        height: image.height() as u32,
        format: ImageFormat::Bgra8,
        data,
    })
}

define_sensor_adapter!(CameraAdapter, SensorType::Camera, Image, image_to_payload);
