pub const SERVICE_NAME: &str = "matching-engine";
pub const DEFAULT_PORT: u16 = 8088;

pub mod order_book;
pub mod shard;

pub type OrderId = [u8; 16];
