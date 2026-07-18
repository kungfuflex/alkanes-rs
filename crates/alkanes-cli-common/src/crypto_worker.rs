// This file defines the Web Worker for cryptographic operations.
// By offloading the expensive PBKDF2 function to a separate thread,
// we prevent the UI from freezing during keystore creation/decryption.

// Refactoring Journal:
// 1. Initial attempt used gloo-worker's `#[worker]` macro, which was from an older, incorrect API version.
// 2. Switched to the `#[gloo_worker::reactor]` macro based on v0.5.0 examples.
// 3. The previous implementation had an incorrect loop structure (`if let Some((msg, id))`)
//    which did not match the `ReactorScope` API.
// 4. Corrected the loop to use `while let Some(request) = scope.next().await` and
//    `scope.send(response).await`, which aligns with the duplex stream pattern of the reactor.

use futures::{SinkExt, StreamExt};
use gloo_worker::reactor::ReactorScope;
use serde::{Deserialize, Serialize};
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Serialize, Deserialize)]
pub enum Request {
    Encrypt {
        data: Vec<u8>,
        passphrase: String,
    },
    Decrypt {
        encrypted_data: Vec<u8>,
        passphrase: String,
        salt: Vec<u8>,
        nonce: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    EncryptSuccess {
        encrypted_data: Vec<u8>,
        salt: Vec<u8>,
        nonce: Vec<u8>,
    },
    DecryptSuccess {
        decrypted_data: Vec<u8>,
    },
    Error(String),
}

#[gloo_worker_macros::reactor]
pub async fn CryptoWorker(mut scope: ReactorScope<Request, Response>) {
    while let Some(request) = scope.next().await {
        let response = match request {
            Request::Encrypt { data, passphrase } => {
                match crate::crypto::encrypt_sync(&data, &passphrase) {
                    Ok((encrypted_data, salt, nonce)) => Response::EncryptSuccess {
                        encrypted_data,
                        salt,
                        nonce,
                    },
                    Err(e) => Response::Error(e.to_string()),
                }
            }
            Request::Decrypt {
                encrypted_data,
                passphrase,
                salt,
                nonce,
            } => match crate::crypto::decrypt_sync(&encrypted_data, &passphrase, &salt, &nonce) {
                Ok(decrypted_data) => Response::DecryptSuccess { decrypted_data },
                Err(e) => Response::Error(e.to_string()),
            },
        };
        if scope.send(response).await.is_err() {
            break;
        }
    }
}