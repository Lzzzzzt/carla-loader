# Sync Engine PDF Requirements (硬约束清单)

> Extracted from spec/04_sync_engine_agent_pdf_spec.md and design diagram

## 1. Event-Driven Trigger Architecture

| Constraint | Description | Source |
|------------|-------------|--------|
| Event-driven | Sync triggered on any new packet arrival | Spec §2.1, §3 |
| Per-sensor buffer | Each sensor has its own ordered queue | Spec §2.1 |
| Reference sensor | Main sensor as time reference (configurable) | Spec §2.1 |
| Output condition | Output only when all required sensors have ≥1 frame | Spec §2.1 |

## 2. IMU Adaptive Window (Δt)

| Constraint | Default Value | Configurable |
|------------|---------------|--------------|
| Minimum window | 20ms | Yes |
| Maximum window | 100ms | Yes |
| Motion intensity | Derived from IMU (linear/angular velocity) | - |
| Window mapping | Higher motion → smaller window | Spec §2.2 |
| Interpolation | Required when window approaches 0 (high IMU rate) | Spec §2.2 |

## 3. AdaKF Time Offset Estimation

| State | Description |
|-------|-------------|
| Per-sensor offset | 1D relative offset to reference clock |
| Observation | Selected frame timestamp vs reference time |
| Update | Kalman filter update to estimate true offset |
| Adaptive | Adjust noise covariance based on residual statistics |

## 4. Output & Eviction Strategy

| Strategy | Behavior | Configurable |
|----------|----------|--------------|
| Output format | `SyncedFrame { t_sync, frames, sync_meta }` | - |
| Used frame removal | Remove consumed frames from buffer | - |
| Expired frame eviction | Configurable (timeout-based) | Yes |
| Missing data handling | `drop` / `empty` / `interpolate` | Yes |

## 5. Observability Requirements

| Metric | Type |
|--------|------|
| `buffer_depth` | Per-sensor buffer size |
| `sync_latency` | Time from input to output |
| `kf_residual` | Kalman filter residual per sensor |
| `dropped_count` | Total dropped packets |
| `out_of_order_count` | Packets arriving out of order |

## 6. Collaboration Constraints

- Sync Engine behavior is **PDF authoritative**
- Only depends on `contracts` crate
- Cannot import `sinks` or `dispatcher`
- Conflicts must be reported via `gap_analysis`

## 7. Required Rust Libraries

Based on research:

| Library | Purpose | Version |
|---------|---------|---------|
| `adskalman` | Kalman filter implementation (nalgebra-based) | 0.17 |
| `nalgebra` | Linear algebra operations | latest |
| `std::collections::BinaryHeap` | Min-heap for ordered buffers | std |
