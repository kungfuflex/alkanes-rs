// This file is part of the deezel project.
// Copyright (c) 2023, Casey Rodarmor, all rights reserved.
// Copyright (c) 2024, The Deezel Developers, all rights reserved.
// Deezel is licensed under the MIT license.
// See LICENSE file in the project root for full license information.

//! This module contains utility functions for the deezel-common crate.
//! It includes functions for hex encoding/decoding, protostone manipulation,
//! and other helper utilities.

/// Converts a slice of bytes to a u128.
pub fn u128_from_slice(slice: &[u8]) -> u128 {
  let mut bytes = [0u8; 16];
  bytes[..slice.len()].copy_from_slice(slice);
  u128::from_le_bytes(bytes)
}
// Chadson v69.69
// This file contains utility functions for cryptographic operations using the Web Crypto API.
// It has been refactored to use asynchronous functions to avoid blocking the UI thread during
// expensive operations like key derivation.
// Source of truth for Web Crypto API usage: wasm-bindgen source for web-sys.

use crate::{DeezelError, Result};
use alloc::vec::Vec;
use js_sys::Object;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::CryptoKey;

/// Derives a key from a passphrase using PBKDF2.
/// This function is async and uses the Web Crypto API. It is intended to be run inside a Web Worker.
pub async fn derive_key_from_passphrase(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
) -> Result<Vec<u8>> {
    // In a worker, `self.crypto()` or `web_sys::crypto()` should be available.
    let subtle = web_sys::window()
        .ok_or_else(|| DeezelError::Crypto("no window".to_string()))?
        .crypto()
        .map_err(|e| DeezelError::Crypto(format!("Failed to get crypto: {:?}", e)))?
        .subtle();

    // 1. Import the passphrase as a raw key material for PBKDF2.
    let import_algorithm = Object::new();
    js_sys::Reflect::set(&import_algorithm, &"name".into(), &"PBKDF2".into()).unwrap();

    let key_material = JsFuture::from(
        subtle
            .import_key_with_object(
                "raw",
                &js_sys::Uint8Array::from(passphrase.as_bytes()),
                &import_algorithm,
                false, // not extractable
                &js_sys::Array::of1(&JsValue::from_str("deriveKey")),
            )
            .map_err(|e| DeezelError::Crypto(format!("Failed to import key: {:?}", e)))?,
    )
    .await
    .map_err(|e| DeezelError::Crypto(format!("Failed to import key future: {:?}", e)))?
    .dyn_into::<CryptoKey>()
    .map_err(|_| DeezelError::Crypto("Failed to cast to CryptoKey".to_string()))?;

    // 2. Define the parameters for PBKDF2 key derivation.
    let hash_algorithm = Object::new();
    js_sys::Reflect::set(&hash_algorithm, &"name".into(), &"SHA-256".into()).unwrap();

    let pbkdf2_params = web_sys::Pbkdf2Params::new(
        "PBKDF2",
        &js_sys::Uint8Array::from(salt),
        iterations,
        &hash_algorithm,
    );

    // 3. Define the algorithm for the derived key (AES-GCM).
    let aes_algo = Object::new();
    js_sys::Reflect::set(&aes_algo, &"name".into(), &"AES-GCM".into()).unwrap();
    js_sys::Reflect::set(&aes_algo, &"length".into(), &256.into()).unwrap();

    // 4. Derive the key.
    let derived_key = JsFuture::from(
        subtle
            .derive_key_with_object_and_object(
                &pbkdf2_params.into(),
                &key_material,
                &aes_algo,
                true, // extractable
                &js_sys::Array::of2(&JsValue::from_str("encrypt"), &JsValue::from_str("decrypt")),
            )
            .map_err(|e| DeezelError::Crypto(format!("Failed to derive key: {:?}", e)))?,
    )
    .await
    .map_err(|e| DeezelError::Crypto(format!("Failed to derive key future: {:?}", e)))?
    .dyn_into::<CryptoKey>()
    .map_err(|_| DeezelError::Crypto("Failed to cast to CryptoKey".to_string()))?;

    // 5. Export the derived key as raw bytes.
    let exported_key = JsFuture::from(
        subtle
            .export_key("raw", &derived_key)
            .map_err(|e| DeezelError::Crypto(format!("Failed to export key: {:?}", e)))?,
    )
    .await
    .map_err(|e| DeezelError::Crypto(format!("Failed to export key future: {:?}", e)))?;

    let key_bytes = js_sys::Uint8Array::new(&exported_key).to_vec();
    Ok(key_bytes)
}