use bitcoin::p2p::address::Address;
use bitcoin::p2p::message_network::VersionMessage;
use bitcoin::p2p::ServiceFlags;
use prost::Message;

#[cfg(target_arch = "wasm32")]
use crate::wasm::{to_arraybuffer_layout};

#[derive(Message)]
pub struct EncodableVersionMessage {
    #[prost(uint32, tag = "1")]
    pub version: u32,
    #[prost(uint64, tag = "2")]
    pub services: u64,
    #[prost(int64, tag = "3")]
    pub timestamp: i64,
    #[prost(message, optional, tag = "4")]
    pub receiver: Option<EncodableAddress>,
    #[prost(message, optional, tag = "5")]
    pub sender: Option<EncodableAddress>,
    #[prost(uint64, tag = "6")]
    pub nonce: u64,
    #[prost(string, tag = "7")]
    pub user_agent: String,
    #[prost(int32, tag = "8")]
    pub start_height: i32,
    #[prost(bool, tag = "9")]
    pub relay: bool,
}

#[derive(Message)]
pub struct EncodableAddress {
    #[prost(uint64, tag = "1")]
    pub services: u64,
    #[prost(bytes, tag = "2")]
    pub address: Vec<u8>,
    #[prost(uint32, tag = "3")]
    pub port: u32,
}

#[cfg(target_arch = "wasm32")]
pub fn to_arraybuffer_layout_for_wasm<T: AsRef<[u8]>>(v: T) -> Vec<u8> {
    let response: Vec<u8> = to_arraybuffer_layout(&v);
    response
}

#[cfg(not(target_arch = "wasm32"))]
pub fn to_arraybuffer_layout_for_wasm<T: AsRef<[u8]>>(v: T) -> Vec<u8> {
    let mut buffer = Vec::<u8>::new();
    buffer.extend_from_slice(&(v.as_ref().len() as u32).to_le_bytes());
    buffer.extend_from_slice(v.as_ref());
    return buffer;
}

impl From<VersionMessage> for EncodableVersionMessage {
    fn from(v: VersionMessage) -> Self {
        Self {
            version: v.version,
            services: v.services.to_u64(),
            timestamp: v.timestamp,
            receiver: Some(v.receiver.into()),
            sender: Some(v.sender.into()),
            nonce: v.nonce,
            user_agent: v.user_agent,
            start_height: v.start_height,
            relay: v.relay,
        }
    }
}

impl From<Address> for EncodableAddress {
    fn from(a: Address) -> Self {
        Self {
            services: a.services.to_u64(),
            address: a.address.iter().flat_map(|&x| x.to_be_bytes()).collect(),
            port: a.port as u32,
        }
    }
}

impl From<EncodableVersionMessage> for VersionMessage {
    fn from(v: EncodableVersionMessage) -> Self {
        Self {
            version: v.version,
            services: ServiceFlags::from(v.services),
            timestamp: v.timestamp,
            receiver: v.receiver.unwrap().into(),
            sender: v.sender.unwrap().into(),
            nonce: v.nonce,
            user_agent: v.user_agent,
            start_height: v.start_height,
            relay: v.relay,
        }
    }
}

impl From<EncodableAddress> for Address {
    fn from(a: EncodableAddress) -> Self {
        let mut address: [u16; 8] = [0; 8];
        for (i, chunk) in a.address.chunks(2).enumerate() {
            address[i] = u16::from_be_bytes([chunk[0], chunk[1]]);
        }
        Self {
            services: ServiceFlags::from(a.services),
            address,
            port: a.port as u16,
        }
    }
}

use crate::{println, stdio::stdout};
use std::fmt::Write;
use std::panic;

pub fn panic_hook(info: &panic::PanicInfo) {
    println!("panic! within WASM: {}", info.to_string());
}
