# Sync Engine Test Plan

> Test cases for verifying multi-sensor synchronization behavior

## 1. Unit Tests

### 1.1 Buffer Operations

| Test Case | Input | Expected Output |
|-----------|-------|-----------------|
| `test_buffer_push_order` | Packets [t=3, t=1, t=2] | Pop order [t=1, t=2, t=3] |
| `test_buffer_capacity` | Push beyond max_size | Oldest evicted, count incremented |
| `test_buffer_timeout` | Old packets + evict(now) | Expired packets removed |
| `test_buffer_peek` | Any packets | Returns earliest, doesn't remove |

### 1.2 Window Calculation

| Test Case | Input | Expected Output |
|-----------|-------|-----------------|
| `test_window_stationary` | IMU at rest (accel ~9.8) | window = 100ms |
| `test_window_high_motion` | IMU high accel+gyro | window ≈ 20ms |
| `test_window_interpolation` | intensity = 0.5 | window = 60ms |

### 1.3 AdaKF

| Test Case | Input | Expected Output |
|-----------|-------|-----------------|
| `test_kf_initial_state` | New estimator | offset = 0.0 |
| `test_kf_converges` | Constant offset +10ms | KF converges to ~10ms |
| `test_kf_tracks_change` | Drifting offset | KF tracks drift |
| `test_adakf_noise_update` | High residual variance | R increases |

## 2. Integration Tests

### 2.1 Normal Sequence

**Setup**: 
- Camera @ 20Hz (50ms interval)
- LiDAR @ 10Hz (100ms interval)  
- IMU @ 100Hz (10ms interval)

**Test `test_normal_sync_sequence`**:
```
Input: 
  t=0.000: cam1, imu1-10
  t=0.050: cam2, imu11-15
  t=0.100: cam3, lidar1, imu16-20
  t=0.150: cam4, imu21-25
  t=0.200: cam5, lidar2, imu26-30

Expected:
  SyncedFrame { t_sync=0.100, frames={cam3, lidar1, imu_latest} }
  SyncedFrame { t_sync=0.200, frames={cam5, lidar2, imu_latest} }
```

### 2.2 Out-of-Order Sequence

**Test `test_out_of_order_reorder`**:
```
Input (arrival order):
  t=0.100: lidar1
  t=0.050: cam2  // arrives late
  t=0.000: cam1  // arrives very late
  t=0.100: cam3
  
Expected:
  - Buffer sorts correctly
  - out_of_order_count = 2
  - Output uses correct temporal order
```

### 2.3 Missing Sensor Data

**Test `test_missing_sensor_drop`** (MissingDataStrategy::Drop):
```
Input:
  t=0.100: cam1, lidar1
  t=0.200: cam2  // lidar missing
  t=0.300: cam3, lidar2

Expected:
  SyncedFrame @ t=0.100
  SyncedFrame @ t=0.300  // t=0.200 skipped
```

**Test `test_missing_sensor_empty`** (MissingDataStrategy::Empty):
```
Expected:
  SyncedFrame @ t=0.100
  SyncedFrame @ t=0.200 { lidar: None }  // outputs with empty
  SyncedFrame @ t=0.300
```

### 2.4 Timestamp Jitter

**Test `test_timestamp_jitter_kf_convergence`**:
```
Input:
  Reference sensor: t=[0.0, 0.1, 0.2, ...]
  Other sensor: t=[0.005, 0.095, 0.208, 0.197, ...]  // jittery
  
Expected:
  KF residual decreases over time
  kf_residuals values in SyncMeta show convergence
```

## 3. Golden File Tests

### 3.1 Deterministic Replay

**File: `tests/fixtures/sync_input_1.json`**
```json
{
  "packets": [
    {"sensor_id": "cam", "timestamp": 0.0, "type": "camera"},
    {"sensor_id": "lidar", "timestamp": 0.05, "type": "lidar"},
    ...
  ]
}
```

**File: `tests/fixtures/sync_output_1.json`**
```json
{
  "frames": [
    {"t_sync": 0.1, "sensors": ["cam", "lidar"]},
    ...
  ]
}
```

**Test `test_golden_file_replay`**:
```rust
#[test]
fn test_golden_file_replay() {
    let input = load_fixture("sync_input_1.json");
    let expected = load_fixture("sync_output_1.json");
    
    let mut engine = SyncEngine::new(config);
    let mut outputs = vec![];
    
    for packet in input.packets {
        if let Some(frame) = engine.push(packet) {
            outputs.push(frame);
        }
    }
    
    assert_eq!(outputs.len(), expected.frames.len());
    for (output, expected) in outputs.iter().zip(expected.frames.iter()) {
        assert_eq!(output.t_sync, expected.t_sync);
        // ... verify other fields
    }
}
```

## 4. Metrics Verification

| Metric | Test | Verification |
|--------|------|--------------|
| `buffer_depth` | Push N packets | depth == N |
| `sync_latency` | Measure time push→output | latency < threshold |
| `kf_residual` | After convergence | residual < 0.01 |
| `dropped_count` | Overflow buffer | count incremented |
| `out_of_order_count` | Send late packets | count incremented |

## 5. Test Commands

```bash
# Run all sync_engine tests
cargo test -p sync_engine

# Run with output for debugging
cargo test -p sync_engine -- --nocapture

# Run specific test
cargo test -p sync_engine test_buffer_push_order

# Run integration tests only
cargo test -p sync_engine --test integration

# Check test coverage
cargo tarpaulin -p sync_engine --out Html
```

## 6. Performance Benchmarks

```bash
# Run benchmarks
cargo bench -p sync_engine

# Expected throughput: >10,000 packets/sec
# Expected latency: <1ms per sync cycle
```
