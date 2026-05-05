#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderId {
    #[prost(bytes = "vec", tag = "1")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}

impl OrderId {
    #[must_use]
    pub fn is_ulid_bytes(&self) -> bool {
        self.value.len() == 16
    }
}
