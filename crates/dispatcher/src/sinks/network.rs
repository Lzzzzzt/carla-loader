//! NetworkSink - UDP fire-and-forget streaming

use contracts::{ContractError, DataSink, SyncedFrame};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::{debug, error, instrument, warn};

/// Serialization format for network transmission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkFormat {
    /// JSON (human-readable, larger)
    #[default]
    Json,
    /// Bincode (binary, compact)
    Bincode,
}

/// Configuration for NetworkSink
#[derive(Debug, Clone)]
pub struct NetworkSinkConfig {
    /// Target address
    pub addr: SocketAddr,
    /// Serialization format
    pub format: NetworkFormat,
    /// Max packet size (UDP typically 65507 for IPv4)
    pub max_packet_size: usize,
}

impl NetworkSinkConfig {
    /// Create config from params map
    pub fn from_params(params: &HashMap<String, String>) -> Result<Self, String> {
        let addr_str = params
            .get("addr")
            .ok_or_else(|| "missing 'addr' parameter".to_string())?;

        let addr: SocketAddr = addr_str
            .parse()
            .map_err(|e| format!("invalid address '{}': {}", addr_str, e))?;

        let format = match params.get("format").map(String::as_str) {
            Some("bincode") => NetworkFormat::Bincode,
            Some("json") | None => NetworkFormat::Json,
            Some(other) => return Err(format!("unknown format '{}'", other)),
        };

        let max_packet_size = params
            .get("max_packet_size")
            .and_then(|s| s.parse().ok())
            .unwrap_or(65000);

        Ok(Self {
            addr,
            format,
            max_packet_size,
        })
    }
}

/// Sink that sends frames over UDP
pub struct NetworkSink {
    name: String,
    config: NetworkSinkConfig,
    socket: Option<UdpSocket>,
}

impl NetworkSink {
    /// Create a new NetworkSink
    #[instrument(name = "network_sink_new", skip(name, config))]
    pub async fn new(name: impl Into<String>, config: NetworkSinkConfig) -> std::io::Result<Self> {
        let name = name.into();
        // Bind to any available port
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(&config.addr).await?;

        debug!(
            sink = %name,
            target = %config.addr,
            "NetworkSink connected"
        );

        Ok(Self {
            name,
            config,
            socket: Some(socket),
        })
    }

    /// Create from params (for factory)
    #[instrument(name = "network_sink_from_params", skip(name, params))]
    pub async fn from_params(
        name: impl Into<String>,
        params: &HashMap<String, String>,
    ) -> Result<Self, ContractError> {
        let config = NetworkSinkConfig::from_params(params)
            .map_err(|e| ContractError::sink_write("network", e))?;

        Self::new(name, config)
            .await
            .map_err(|e| ContractError::SinkConnection {
                sink_name: "network".to_string(),
                message: e.to_string(),
            })
    }

    fn serialize_frame(&self, frame: &SyncedFrame) -> Result<Vec<u8>, String> {
        // Serialize the full frame
        match self.config.format {
            NetworkFormat::Json => {
                serde_json::to_vec(frame).map_err(|e| format!("json error: {}", e))
            }
            NetworkFormat::Bincode => {
                bincode::serialize(frame).map_err(|e| format!("bincode error: {}", e))
            }
        }
    }

    fn socket(&self) -> Result<&UdpSocket, ContractError> {
        self.socket
            .as_ref()
            .ok_or_else(|| ContractError::sink_write(&self.name, "socket not connected"))
    }

    fn prepare_payload(&self, frame: &SyncedFrame) -> Result<Vec<u8>, ContractError> {
        let data = self
            .serialize_frame(frame)
            .map_err(|e| ContractError::sink_write(&self.name, e))?;

        if data.len() > self.config.max_packet_size {
            warn!(
                sink = %self.name,
                size = data.len(),
                max = self.config.max_packet_size,
                "Packet too large, truncating"
            );
        }

        Ok(data)
    }

    async fn transmit(&self, socket: &UdpSocket, data: &[u8], frame_id: u64) {
        match socket.send(data).await {
            Ok(sent) => {
                debug!(sink = %self.name, frame_id, bytes = sent, "Sent");
            }
            Err(e) => {
                // Log but don't fail - UDP is best-effort
                error!(sink = %self.name, error = %e, "UDP send failed");
            }
        }
    }
}

impl DataSink for NetworkSink {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(
        name = "network_sink_write",
        skip(self, frame),
        fields(sink = %self.name, frame_id = frame.frame_id)
    )]
    async fn write(&mut self, frame: &SyncedFrame) -> Result<(), ContractError> {
        let socket = self.socket()?;
        let data = self.prepare_payload(frame)?;
        self.transmit(socket, &data, frame.frame_id).await;
        Ok(())
    }

    #[instrument(name = "network_sink_flush", skip(self))]
    async fn flush(&mut self) -> Result<(), ContractError> {
        // UDP doesn't buffer
        Ok(())
    }

    #[instrument(name = "network_sink_close", skip(self))]
    async fn close(&mut self) -> Result<(), ContractError> {
        self.socket = None;
        debug!(sink = %self.name, "NetworkSink closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::SyncMeta;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_network_sink_config_parsing() {
        let mut params = HashMap::new();
        params.insert("addr".to_string(), "127.0.0.1:9999".to_string());
        params.insert("format".to_string(), "json".to_string());

        let config = NetworkSinkConfig::from_params(&params).unwrap();
        assert_eq!(config.addr.port(), 9999);
        assert_eq!(config.format, NetworkFormat::Json);
    }

    #[tokio::test]
    async fn test_network_sink_create() {
        let config = NetworkSinkConfig {
            addr: "127.0.0.1:19999".parse().unwrap(),
            format: NetworkFormat::Json,
            max_packet_size: 65000,
        };

        let sink = NetworkSink::new("test_net", config).await;
        // Should succeed even if no receiver (UDP doesn't care)
        assert!(sink.is_ok());
    }

    #[tokio::test]
    async fn test_network_sink_write() {
        let config = NetworkSinkConfig {
            addr: "127.0.0.1:19998".parse().unwrap(),
            format: NetworkFormat::Json,
            max_packet_size: 65000,
        };

        let mut sink = NetworkSink::new("test_net", config).await.unwrap();
        let frame = SyncedFrame {
            t_sync: 1.0,
            frame_id: 1,
            frames: HashMap::new(),
            sync_meta: SyncMeta::default(),
        };

        // Should not fail even with no receiver
        let result = sink.write(&frame).await;
        assert!(result.is_ok());
    }
}
