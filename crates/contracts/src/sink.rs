//! DataSink trait - Dispatcher 输出端接口
//!
//! 定义 Sink 的抽象接口。

use crate::{ContractError, SyncedFrame};

/// 数据输出端 trait
///
/// 所有 sink 实现必须实现此 trait。
#[trait_variant::make(DataSink: Send)]
pub trait LocalDataSink {
    /// Sink 名称 (用于日志/指标)
    fn name(&self) -> &str;

    /// 写入同步帧
    ///
    /// # Errors
    /// 返回写入错误 (需包含上下文)
    async fn write(&mut self, frame: &SyncedFrame) -> Result<(), ContractError>;

    /// 刷新缓冲区 (若有)
    async fn flush(&mut self) -> Result<(), ContractError>;

    /// 关闭 sink
    async fn close(&mut self) -> Result<(), ContractError>;
}
