//! IMU adaptive window calculation.

use contracts::ImuData;

use crate::WindowConfig;

/// Calculate motion intensity from IMU data
///
/// Returns a value between 0.0 (stationary) and 1.0 (high motion)
pub fn compute_motion_intensity(imu: &ImuData) -> f64 {
    // Linear acceleration magnitude (excluding gravity)
    let linear_mag =
        (imu.accelerometer.x.powi(2) + imu.accelerometer.y.powi(2) + imu.accelerometer.z.powi(2))
            .sqrt();

    // Angular velocity magnitude
    let angular_mag =
        (imu.gyroscope.x.powi(2) + imu.gyroscope.y.powi(2) + imu.gyroscope.z.powi(2)).sqrt();

    // Normalize:
    // - Gravity is ~9.8 m/s², so we subtract it and normalize remaining to 5 m/s² max
    // - Typical driving angular velocity is ~0.5 rad/s, normalize to 1.0 rad/s max
    let linear_normalized = ((linear_mag - 9.8).abs() / 5.0).clamp(0.0, 1.0);
    let angular_normalized = (angular_mag / 1.0).clamp(0.0, 1.0);

    // Combined intensity (weighted average)
    ((linear_normalized + angular_normalized) / 2.0).clamp(0.0, 1.0)
}

/// Blend IMU-derived intensity with buffer pressure for a single knob.
///
/// `buffer_pressure` is expected to be 0-1, representing how full the queues are.
pub fn fuse_motion_pressure(imu_intensity: f64, buffer_pressure: f64) -> f64 {
    let imu = imu_intensity.clamp(0.0, 1.0);
    let pressure = buffer_pressure.clamp(0.0, 1.0);
    (imu * 0.7 + pressure * 0.3).clamp(0.0, 1.0)
}

/// Compute window size based on motion intensity
///
/// Higher motion → smaller window (tighter sync required)
/// Lower motion → larger window (can tolerate more offset)
pub fn compute_window_size(intensity: f64, config: &WindowConfig) -> f64 {
    // Linear interpolation: intensity=0 → max_window, intensity=1 → min_window
    let range = config.max_ms - config.min_ms;
    let window_ms = config.max_ms - (intensity * range);
    window_ms / 1000.0 // Convert to seconds
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Vector3;

    #[test]
    fn test_motion_intensity_stationary() {
        let imu = ImuData {
            accelerometer: Vector3 {
                x: 0.0,
                y: 0.0,
                z: 9.8,
            },
            gyroscope: Vector3::default(),
            compass: 0.0,
        };

        let intensity = compute_motion_intensity(&imu);
        assert!(intensity < 0.1, "Stationary IMU should have low intensity");
    }

    #[test]
    fn test_motion_intensity_high_motion() {
        let imu = ImuData {
            accelerometer: Vector3 {
                x: 5.0,
                y: 0.0,
                z: 9.8,
            },
            gyroscope: Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            compass: 0.0,
        };

        let intensity = compute_motion_intensity(&imu);
        assert!(
            intensity > 0.5,
            "High motion IMU should have high intensity"
        );
    }

    #[test]
    fn test_window_size_stationary() {
        let config = WindowConfig::default();
        let window = compute_window_size(0.0, &config);
        assert!(
            (window - 0.1).abs() < 0.001,
            "Stationary → max window 100ms"
        );
    }

    #[test]
    fn test_window_size_high_motion() {
        let config = WindowConfig::default();
        let window = compute_window_size(1.0, &config);
        assert!(
            (window - 0.02).abs() < 0.001,
            "High motion → min window 20ms"
        );
    }

    #[test]
    fn test_window_size_interpolation() {
        let config = WindowConfig::default();
        let window = compute_window_size(0.5, &config);
        assert!((window - 0.06).abs() < 0.001, "0.5 intensity → 60ms window");
    }

    #[test]
    fn test_fuse_motion_pressure_bounds() {
        assert!(fuse_motion_pressure(0.0, 0.0) <= 0.01);
        assert!((fuse_motion_pressure(1.0, 1.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fuse_motion_pressure_weighting() {
        let fused = fuse_motion_pressure(0.2, 0.8);
        // 0.2*0.7 + 0.8*0.3 = 0.38
        assert!((fused - 0.38).abs() < 1e-6);
    }
}
