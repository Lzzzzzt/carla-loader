# Observability Survey

## 1. Current State

### Logging & Tracing
- **Library**: `tracing` (v0.1) and `tracing-subscriber` (v0.3) are already in use.
- **Usage**:
  - `crates/observability` already depends on `tracing` and `tracing-subscriber` (with `env-filter`, `json` features).
  - `ingestion` uses `tracing`.
- **Format**: `tracing-subscriber` configured with `json` feature suggests JSON output capability is available.

### Metrics
- **Library**: None currently used.
- **Findings**: `grep` for `prometheus`, `opentelemetry`, `metrics` returned no results in usages.
- **Gap**: Need to introduce a metrics facade and exporter.

### Error Handling
- **Library**: `thiserror` is widely used in library crates (`ingestion`, `contracts`, `actor_factory`, `dispatcher`).
- **Pattern**: Custom error enums deriving `Error`.
- **Anyhow**: Not used, which is good for libraries.

## 2. Recommendations

### Metrics Selection
- **Role**: `metrics` (facade) + `metrics-exporter-prometheus`.
- **Reasoning**:
  - `metrics` crate provides a lightweight facade similar to `tracing`.
  - `metrics-exporter-prometheus` allows scraping via HTTP or pushing to Gateway.
  - Avoids full OpenTelemetry heavy dependencies unless complex distributed traces are needed immediately (specification warns against binary bloat).

### Standardization
- **Spans**: Establish `component`, `sensor_id`, `frame_id` as standard fields.
- **Errors**: Continue using `thiserror`. Ensure error types impl `std::error::Error` and are logged with `tracing::error!` in top-level loops.

### Crates Structure
- Use existing `observability` crate to export setup functions and re-export `tracing` / `metrics` macros if needed for convenience (or just let crates depend on `tracing`/`metrics` directly).
