//! Adaptive Kalman Filter (AdaKF) for time offset estimation.
//!
//! Implements a lightweight 2-state (offset + drift) Kalman filter with
//! EWMA-based measurement noise tuning and load-aware process noise scaling.

use std::collections::VecDeque;

use crate::AdaKFConfig;

const MIN_DT: f64 = 1e-3;
const DEFAULT_ALPHA: f64 = 0.85;

/// Adaptive Kalman Filter for per-sensor time offset estimation
///
/// State vector x = [offset, drift]^T where:
/// - `offset` is the static bias relative to reference clock
/// - `drift` captures first-order rate change of the offset
///
/// Transition matrix F = [[1, Δt], [0, 1]]
/// Observation matrix H = [1, 0]
#[derive(Debug, Clone)]
pub struct AdaKF {
    /// Current state estimate (offset, drift)
    state: [f64; 2],
    /// State covariance matrix
    covariance: [[f64; 2]; 2],
    /// Base process noise terms (scaled per load)
    base_q_offset: f64,
    base_q_drift: f64,
    /// Measurement noise baseline
    base_r: f64,
    /// Current measurement noise
    r: f64,
    /// EWMA of residual variance (for R adaptation)
    ewma_variance: f64,
    /// Residual history for diagnostics
    residual_window: VecDeque<f64>,
    /// Max residual history length
    window_size: usize,
    /// EWMA smoothing factor
    alpha: f64,
    /// Expected sampling interval (seconds)
    expected_interval: f64,
}

impl AdaKF {
    /// Create a new AdaKF estimator
    pub fn new(config: &AdaKFConfig) -> Self {
        let window_size = config.residual_window.max(3);
        let base_q_offset = config.process_noise.max(1e-9);
        let base_q_drift = (config.process_noise * 0.1).max(1e-9);
        let base_r = config.measurement_noise.max(1e-9);
        let expected_interval = config.expected_interval.unwrap_or(0.05).max(MIN_DT);

        Self {
            state: [config.initial_offset, 0.0],
            covariance: [[1.0, 0.0], [0.0, 1.0]],
            base_q_offset,
            base_q_drift,
            base_r,
            r: base_r,
            ewma_variance: base_r,
            residual_window: VecDeque::with_capacity(window_size),
            window_size,
            alpha: DEFAULT_ALPHA,
            expected_interval,
        }
    }

    /// Update the filter with a new observation.
    ///
    /// * `observation` - observed time difference `t_sensor - t_reference` (seconds)
    /// * `dt` - elapsed reference time since last update (seconds)
    /// * `load_index` - 0-1 hint derived from buffer pressure
    pub fn update(&mut self, observation: f64, dt: f64, load_index: f64) -> (f64, f64) {
        let dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            self.expected_interval
        }
        .max(MIN_DT);

        // ===== Predict step =====
        let offset_pred = self.state[0] + dt * self.state[1];
        let drift_pred = self.state[1];

        // Process noise grows with buffer pressure to react faster when queues spike
        let scale = 1.0 + load_index.clamp(0.0, 1.0);
        let q_offset = self.base_q_offset * scale;
        let q_drift = self.base_q_drift * scale;

        // Covariance prediction for 2x2 state
        let p00 = self.covariance[0][0];
        let p01 = self.covariance[0][1];
        let p11 = self.covariance[1][1];

        let pred00 = p00 + 2.0 * dt * p01 + dt * dt * p11 + q_offset;
        let pred01 = p01 + dt * p11;
        let pred11 = p11 + q_drift;

        // ===== Update step =====
        let residual = observation - offset_pred;
        let s = pred00 + self.r;
        let k0 = pred00 / s;
        let k1 = pred01 / s;

        let new_offset = offset_pred + k0 * residual;
        let new_drift = drift_pred + k1 * residual;

        let new_p00 = (1.0 - k0) * pred00;
        let new_p01 = (1.0 - k0) * pred01;
        let new_p11 = pred11 - k1 * pred01;

        self.state = [new_offset, new_drift];
        self.covariance = [[new_p00.max(0.0), new_p01], [new_p01, new_p11.max(0.0)]];

        self.record_residual(residual);
        self.update_measurement_noise(residual);

        (self.state[0], residual)
    }

    /// Current offset estimate
    pub fn offset(&self) -> f64 {
        self.state[0]
    }

    /// Current drift estimate (seconds per second)
    #[allow(dead_code)]
    pub fn drift(&self) -> f64 {
        self.state[1]
    }

    /// Current uncertainty of offset component
    #[allow(dead_code)]
    pub fn uncertainty(&self) -> f64 {
        self.covariance[0][0]
    }

    fn record_residual(&mut self, residual: f64) {
        self.residual_window.push_back(residual);
        if self.residual_window.len() > self.window_size {
            self.residual_window.pop_front();
        }
    }

    fn update_measurement_noise(&mut self, residual: f64) {
        self.ewma_variance =
            self.alpha * self.ewma_variance + (1.0 - self.alpha) * residual.powi(2);
        let r_min = self.base_r * 0.1;
        let r_max = self.base_r * 10.0;
        self.r = self.ewma_variance.clamp(r_min, r_max);
    }

    /// Get the latest residuals for diagnostics
    #[allow(dead_code)]
    pub fn recent_residuals(&self) -> impl Iterator<Item = &f64> {
        self.residual_window.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adakf_initial_state() {
        let config = AdaKFConfig::default();
        let kf = AdaKF::new(&config);
        assert_eq!(kf.offset(), 0.0);
    }

    #[test]
    fn test_adakf_converges_to_constant_offset() {
        let config = AdaKFConfig {
            initial_offset: 0.0,
            process_noise: 0.0001,
            measurement_noise: 0.001,
            residual_window: 10,
            expected_interval: None,
        };

        let mut kf = AdaKF::new(&config);

        // Feed constant offset observations
        let true_offset = 0.01; // 10ms offset
        for _ in 0..50 {
            kf.update(true_offset, 0.05, 0.0);
        }

        // Should converge close to true offset
        let estimated = kf.offset();
        assert!(
            (estimated - true_offset).abs() < 0.001,
            "Expected ~{}, got {}",
            true_offset,
            estimated
        );
    }

    #[test]
    fn test_adakf_tracks_changing_offset() {
        let config = AdaKFConfig {
            initial_offset: 0.0,
            process_noise: 0.001, // Higher process noise for tracking
            measurement_noise: 0.001,
            residual_window: 10,
            expected_interval: None,
        };

        let mut kf = AdaKF::new(&config);

        // Feed slowly drifting offset
        for i in 0..100 {
            let observation = (i as f64) * 0.0001; // Slowly increasing
            kf.update(observation, 0.05, 0.0);
        }

        // Should track the drift
        let estimated = kf.offset();
        assert!(
            estimated > 0.005,
            "Should have tracked positive drift, got {}",
            estimated
        );
    }

    #[test]
    fn test_adakf_handles_noisy_observations() {
        let config = AdaKFConfig {
            initial_offset: 0.0,
            process_noise: 0.0001,
            measurement_noise: 0.01,
            residual_window: 20,
            expected_interval: None,
        };

        let mut kf = AdaKF::new(&config);
        let true_offset = 0.05;

        // Feed noisy observations around true offset
        for i in 0..100 {
            let noise = ((i % 10) as f64 - 5.0) * 0.002; // ±10ms noise
            kf.update(true_offset + noise, 0.05, 0.0);
        }

        // Should converge close to true offset despite noise
        let estimated = kf.offset();
        assert!(
            (estimated - true_offset).abs() < 0.01,
            "Expected ~{}, got {}",
            true_offset,
            estimated
        );
    }
}
