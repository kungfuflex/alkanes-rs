use crate::network::get_network;
use anyhow::Result;
use bitcoin::consensus::{
    deserialize_partial,
    encode::{Decodable, Encodable},
};
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, Txid};
use metashrew_support::utils::{is_empty, remaining_slice};
use ordinals::varint;
use std::{io::BufRead};
use bech32::Hrp;
use bitcoin::Script;
use metashrew_support::address::{AddressEncoding, Payload};

pub fn to_address_str(script: &Script) -> Result<String> {
    let config = get_network();
    Ok(AddressEncoding {
        p2pkh_prefix: config.p2pkh_prefix,
        p2sh_prefix: config.p2sh_prefix,
        hrp: Hrp::parse_unchecked(&config.bech32_prefix),
        payload: &Payload::from_script(script)?,
    }
    .to_string())
}

pub fn split_bytes(bytes: &[u8]) -> Vec<u128> {
    let mut result = Vec::new();
    for chunk in bytes.chunks(16) {
        let mut arr = [0u8; 16];
        arr[..chunk.len()].copy_from_slice(chunk);
        result.push(u128::from_le_bytes(arr));
    }
    result
}

pub fn consensus_encode<T: Encodable>(v: &T) -> Result<Vec<u8>> {
    let mut result = Vec::<u8>::new();
    <T as Encodable>::consensus_encode::<Vec<u8>>(v, &mut result)?;
    Ok(result)
}

pub fn consensus_decode<T: Decodable>(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<T> {
    let slice = &cursor.get_ref()[cursor.position() as usize..cursor.get_ref().len() as usize];
    let deserialized: (T, usize) = deserialize_partial(slice)?;
    cursor.consume(deserialized.1);
    Ok(deserialized.0)
}

pub fn tx_hex_to_txid(s: &str) -> Result<Txid> {
    Ok(Txid::from_byte_array(
        <Vec<u8> as AsRef<[u8]>>::as_ref(
            &hex::decode(s)?.iter().cloned().rev().collect::<Vec<u8>>(),
        )
        .try_into()?,
    ))
}

pub fn reverse_txid(v: &Txid) -> Txid {
    let reversed_bytes: Vec<u8> = v
        .clone()
        .as_byte_array()
        .into_iter()
        .map(|v| v.clone())
        .rev()
        .collect::<Vec<u8>>();
    let reversed_bytes_ref: &[u8] = &reversed_bytes;
    Txid::from_byte_array(reversed_bytes_ref.try_into().unwrap())
}

pub fn outpoint_encode(v: &OutPoint) -> Result<Vec<u8>> {
    consensus_encode(&v)
}

pub fn decode_varint_list(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Vec<u128>> {
    let mut result: Vec<u128> = vec![];
    while !is_empty(cursor) {
        let (n, sz) = varint::decode(remaining_slice(cursor))?;
        cursor.consume(sz);
        result.push(n);
    }
    Ok(result)
}

/// returns the values in a LEB encoded stream
pub fn encode_varint_list(values: &Vec<u128>) -> Vec<u8> {
    let mut result = Vec::<u8>::new();
    for value in values {
        varint::encode_to_vec(*value, &mut result);
    }
    result
}

pub fn field_to_name(data: &u128) -> String {
    let mut v = data + 1; // Increment by 1
    let mut result = String::new();
    let twenty_six: u128 = 26;

    while v > 0 {
        let mut y = (v % twenty_six) as u32;
        if y == 0 {
            y = 26;
        }

        // Convert number to character (A-Z, where A is 65 in ASCII)
        result.insert(0, char::from_u32(64 + y).unwrap());

        v -= 1; // Decrement v by 1
        v /= twenty_six; // Divide v by 26 for next iteration
    }

    result
}

