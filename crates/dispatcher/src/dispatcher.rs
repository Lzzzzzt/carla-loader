//! Dispatcher - main loop for fan-out to sinks

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, instrument};

use contracts::{SinkConfig, SinkType, SyncedFrame};

use crate::error::DispatcherError;
use crate::handle::SinkHandle;
use crate::metrics::MetricsSnapshot;
use crate::sinks::{FileSink, LogSink, NetworkSink};

/// Dispatcher configuration
#[derive(Debug, Clone)]
pub struct DispatcherConfig {
    /// Sink configurations
    pub sinks: Vec<SinkConfig>,
}

/// Builder for creating a Dispatcher
pub struct DispatcherBuilder {
    config: DispatcherConfig,
    input_rx: mpsc::Receiver<SyncedFrame>,
}

impl DispatcherBuilder {
    /// Create a new DispatcherBuilder
    pub fn new(config: DispatcherConfig, input_rx: mpsc::Receiver<SyncedFrame>) -> Self {
        Self { config, input_rx }
    }

    /// Build and start the dispatcher
    #[instrument(name = "dispatcher_builder_build", skip(self))]
    pub async fn build(self) -> Result<Dispatcher, DispatcherError> {
        let handles = Self::initialize_handles(&self.config).await?;

        Ok(Dispatcher {
            handles,
            input_rx: self.input_rx,
        })
    }

    #[instrument(
        name = "dispatcher_initialize_handles",
        skip(config),
        fields(sink_count = config.sinks.len())
    )]
    async fn initialize_handles(
        config: &DispatcherConfig,
    ) -> Result<Vec<SinkHandle>, DispatcherError> {
        let mut handles = Vec::with_capacity(config.sinks.len());
        for sink_config in &config.sinks {
            handles.push(create_sink_handle(sink_config).await?);
        }
        Ok(handles)
    }
}

/// Create a SinkHandle from configuration
#[instrument(
    name = "dispatcher_create_sink_handle",
    skip(config),
    fields(sink = %config.name, sink_type = ?config.sink_type)
)]
async fn create_sink_handle(config: &SinkConfig) -> Result<SinkHandle, DispatcherError> {
    match config.sink_type {
        SinkType::Log => {
            let sink = LogSink::new(&config.name);
            Ok(SinkHandle::spawn(sink, config.queue_capacity))
        }
        SinkType::File => {
            let sink = FileSink::from_params(&config.name, &config.params)
                .map_err(|e| DispatcherError::sink_creation(&config.name, e.to_string()))?;
            Ok(SinkHandle::spawn(sink, config.queue_capacity))
        }
        SinkType::Network => {
            let sink = NetworkSink::from_params(&config.name, &config.params)
                .await
                .map_err(|e| DispatcherError::sink_creation(&config.name, e.to_string()))?;
            Ok(SinkHandle::spawn(sink, config.queue_capacity))
        }
    }
}

/// The main Dispatcher that fans out frames to sinks
pub struct Dispatcher {
    handles: Vec<SinkHandle>,
    input_rx: mpsc::Receiver<SyncedFrame>,
}

impl Dispatcher {
    /// Create a dispatcher with custom sink handles (for testing)
    pub fn with_handles(handles: Vec<SinkHandle>, input_rx: mpsc::Receiver<SyncedFrame>) -> Self {
        Self { handles, input_rx }
    }

    /// Get metrics for all sinks
    pub fn metrics(&self) -> Vec<(String, MetricsSnapshot)> {
        self.handles
            .iter()
            .map(|h| (h.name().to_string(), h.metrics().snapshot()))
            .collect()
    }

    /// Run the dispatcher main loop
    ///
    /// Consumes frames from input and fans out to all sinks.
    /// Returns when input channel is closed.
    #[instrument(name = "dispatcher_run", skip(self))]
    pub async fn run(mut self) {
        info!(sinks = self.handles.len(), "Dispatcher started");

        let mut frame_count: u64 = 0;

        while let Some(frame) = self.input_rx.recv().await {
            frame_count += 1;
            self.dispatch_frame(&frame);

            if frame_count.is_multiple_of(100) {
                debug!(frames = frame_count, "Dispatcher progress");
            }
        }

        info!(
            frames = frame_count,
            "Dispatcher input closed, shutting down"
        );

        Self::shutdown_handles(self.handles).await;

        info!("Dispatcher shutdown complete");
    }

    /// Spawn the dispatcher as a background task
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    fn dispatch_frame(&self, frame: &SyncedFrame) {
        for handle in &self.handles {
            handle.try_send(frame.clone());
        }
    }

    async fn shutdown_handles(handles: Vec<SinkHandle>) {
        for handle in handles {
            handle.shutdown().await;
        }
    }
}

/// Convenience function to create a dispatcher from sink configs
#[instrument(name = "dispatcher_create", skip(sink_configs, input_rx))]
pub async fn create_dispatcher(
    sink_configs: Vec<SinkConfig>,
    input_rx: mpsc::Receiver<SyncedFrame>,
) -> Result<Dispatcher, DispatcherError> {
    let config = DispatcherConfig {
        sinks: sink_configs,
    };
    DispatcherBuilder::new(config, input_rx).build().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::SyncMeta;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_dispatcher_fanout() {
        let (input_tx, input_rx) = mpsc::channel(10);

        // Create log sinks for testing
        let sink1 = LogSink::new("sink1");
        let sink2 = LogSink::new("sink2");

        let handles = vec![SinkHandle::spawn(sink1, 10), SinkHandle::spawn(sink2, 10)];

        let dispatcher = Dispatcher::with_handles(handles, input_rx);
        let handle = dispatcher.spawn();

        // Send some frames
        for i in 0..5 {
            let frame = SyncedFrame {
                t_sync: i as f64,
                frame_id: i,
                frames: HashMap::new(),
                sync_meta: SyncMeta::default(),
            };
            input_tx.send(frame).await.unwrap();
        }

        // Close input channel
        drop(input_tx);

        // Wait for dispatcher to finish
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_create_dispatcher_from_config() {
        let (input_tx, input_rx) = mpsc::channel(10);

        let configs = vec![SinkConfig {
            name: "test_log".to_string(),
            sink_type: SinkType::Log,
            queue_capacity: 50,
            params: HashMap::new(),
        }];

        let dispatcher = create_dispatcher(configs, input_rx).await.unwrap();
        let handle = dispatcher.spawn();

        // Send a frame
        let frame = SyncedFrame {
            t_sync: 1.0,
            frame_id: 1,
            frames: HashMap::new(),
            sync_meta: SyncMeta::default(),
        };
        input_tx.send(frame).await.unwrap();

        drop(input_tx);
        handle.await.unwrap();
    }
}
