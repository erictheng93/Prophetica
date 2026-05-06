pub const SERVICE_NAME: &str = "matching-engine";
pub const DEFAULT_PORT: u16 = 8088;
pub const DEFAULT_GRPC_PORT: u16 = 9088;

pub mod api;
pub mod order_book;
pub mod projection;
pub mod service;
pub mod shard;
pub mod snapshot;
pub mod wal;

pub type OrderId = [u8; 16];
