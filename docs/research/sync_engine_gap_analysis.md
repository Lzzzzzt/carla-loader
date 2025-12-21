# Sync Engine Gap Analysis

> Comparing `contracts` definitions with PDF requirements

## 1. Existing Contracts Assessment

### ✅ Adequate Types

| Type | Location | Status |
|------|----------|--------|
| `SyncedFrame` | `contracts/src/sync.rs` | ✅ Has `t_sync`, `frame_id`, `frames`, `sync_meta` |
| `SyncMeta` | `contracts/src/sync.rs` | ✅ Has `reference_sensor_id`, `window_size`, `time_offsets`, `kf_residuals`, `missing_sensors`, `dropped_count`, `out_of_order_count` |
| `SensorPacket` | `contracts/src/sensor.rs` | ✅ Has `sensor_id`, `sensor_type`, `timestamp`, `frame_id`, `payload` |
| `BufferStats` | `contracts/src/sync.rs` | ✅ Has `buffer_depths`, `total_packets`, `oldest_timestamp`, `newest_timestamp` |

### ⚠️ Minor Enhancements Needed

| Item | Issue | Proposed Action |
|------|-------|-----------------|
| `SyncedPacket` | Not currently used in output | Can be used internally for frame selection |
| IMU window params | Not in SyncMeta | Add optional `motion_intensity: Option<f64>` to SyncMeta |

## 2. Missing Configurations (Need to Add)

For the sync engine config, we need:

```rust
pub struct SyncEngineConfig {
    /// Reference sensor ID (main clock source)
    pub reference_sensor_id: String,
    
    /// Required sensor types for sync output
    pub required_sensors: Vec<SensorType>,
    
    /// IMU window bounds
    pub min_window_ms: f64,  // default: 20.0
    pub max_window_ms: f64,  // default: 100.0
    
    /// Missing data strategy
    pub missing_strategy: MissingDataStrategy,
    
    /// Buffer timeout (seconds) before eviction
    pub buffer_timeout_s: f64,
    
    /// Maximum buffer size per sensor
    pub max_buffer_size: usize,
}

pub enum MissingDataStrategy {
    Drop,       // Skip output if missing
    Empty,      // Output with empty slot
    Interpolate, // Interpolate from adjacent frames
}
```

## 3. AdaKF Implementation Requirements

The adaptive Kalman filter for time offset estimation needs:

1. **State Model**: 1D state per sensor (time offset)
2. **Transition Model**: Identity (offset is quasi-static)
3. **Observation Model**: `z = t_selected - t_reference`
4. **Adaptive Component**: 
   - Track residual `e = z - H*x`
   - Update measurement noise `R` based on residual variance
   - Update process noise `Q` when offset drifts detected

## 4. Gap Summary

| Requirement | Status | Action |
|-------------|--------|--------|
| Per-sensor buffer queue | 🔴 Missing | Implement with BinaryHeap |
| IMU adaptive window | 🔴 Missing | Implement motion intensity → window mapping |
| AdaKF offset estimator | 🔴 Missing | Implement with adskalman |
| Frame selection logic | 🔴 Missing | Implement window-based selection |
| Eviction strategy | 🔴 Missing | Implement timeout-based cleanup |
| Metrics export | 🔴 Missing | Add to sync pipeline |

## 5. Proposed Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                       SyncEngine                                │
├────────────────────────────────────────────────────────────────┤
│ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                │
│ │ CamBuffer   │ │ LidarBuffer │ │ IMUBuffer   │ ...            │
│ │ (MinHeap)   │ │ (MinHeap)   │ │ (MinHeap)   │                │
│ └──────┬──────┘ └──────┬──────┘ └──────┬──────┘                │
│        │               │               │                        │
│        └───────────────┼───────────────┘                        │
│                        ▼                                        │
│              ┌─────────────────────┐                            │
│              │ WindowCalculator    │                            │
│              │ (IMU-based Δt)      │                            │
│              └─────────┬───────────┘                            │
│                        ▼                                        │
│              ┌─────────────────────┐                            │
│              │ FrameSelector       │                            │
│              │ (within window)     │                            │
│              └─────────┬───────────┘                            │
│                        ▼                                        │
│              ┌─────────────────────┐                            │
│              │ AdaKF Estimator     │                            │
│              │ (per-sensor offset) │                            │
│              └─────────┬───────────┘                            │
│                        ▼                                        │
│              ┌─────────────────────┐                            │
│              │ SyncedFrame Output  │                            │
│              └─────────────────────┘                            │
└────────────────────────────────────────────────────────────────┘
```

## 6. Recommended Libraries

| Library | Purpose | Cargo Declaration |
|---------|---------|-------------------|
| `adskalman` | Kalman filter with nalgebra | `adskalman = "0.17"` |
| `nalgebra` | Matrix operations | `nalgebra = "0.33"` |
| `metrics` | Observability | Already in workspace |
