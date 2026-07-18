// Debug test file - all imports commented out since tests are disabled
// use deezel_common::pgp_rpgp::RpgpPgpProvider;
// use deezel_common::traits::PgpProvider;
// use deezel_rpgp::composed::Deserializable;

// #[tokio::test]
// async fn debug_decrypt_and_verify() {
//     let provider = RpgpPgpProvider::new();
//     let recipient_keypair = provider
//         .generate_keypair("recipient", None)
//         .await
//         .unwrap();
//     let sender_keypair = provider
//         .generate_keypair("sender", None)
//         .await
//         .unwrap();

//     let data = b"hello world";
    
//     // Test encrypt_and_sign
//     println!("=== Testing encrypt_and_sign ===");
//     let encrypted = provider
//         .encrypt_and_sign(
//             data,
//             &[recipient_keypair.public_key.clone()],
//             &sender_keypair.private_key,
//             None,
//             true, // armored
//         )
//         .await
//         .unwrap();
    
//     println!("Encrypted+signed data length: {}", encrypted.len());
//     println!("Encrypted+signed data (first 100 chars): {}",
//         String::from_utf8_lossy(&encrypted[..100.min(encrypted.len())]));
    
//     // Try to parse the message to see its structure
//     println!("\n=== Analyzing message structure ===");
//     let encrypted_str = String::from_utf8(encrypted.clone()).unwrap();
    
//     // Try to parse with deezel_rpgp
//     match deezel_rpgp::composed::Message::from_string(&encrypted_str) {
//         Ok((message, _headers)) => {
//             println!("Successfully parsed message");
//             println!("Message type: {:?}", std::mem::discriminant(&message));
            
//             // Try to decrypt step by step
//             println!("\n=== Attempting decryption ===");
            
//             let (secret_key, _headers) = deezel_rpgp::composed::SignedSecretKey::from_string(
//                 &String::from_utf8(recipient_keypair.private_key.key_data.clone()).unwrap(),
//             ).unwrap();
            
//             let ring = deezel_rpgp::composed::TheRing {
//                 secret_keys: vec![&secret_key],
//                 key_passwords: vec![],
//                 ..Default::default()
//             };
            
//             match message.decrypt_the_ring(ring, true) {
//                 Ok((mut decrypted_message, _ring_result)) => {
//                     println!("Decryption successful!");
//                     println!("Decrypted message type: {:?}", std::mem::discriminant(&decrypted_message));
                    
//                     // Try to get data
//                     match decrypted_message.as_data_vec() {
//                         Ok(data_vec) => {
//                             println!("Data extraction successful: {:?}", String::from_utf8_lossy(&data_vec));
//                         }
//                         Err(e) => {
//                             println!("Data extraction failed: {}", e);
//                         }
//                     }
//                 }
//                 Err(e) => {
//                     println!("Decryption failed: {}", e);
//                 }
//             }
//         }
//         Err(e) => {
//             println!("Failed to parse message: {}", e);
//         }
//     }
    
//     // Now test the full decrypt_and_verify function
//     println!("\n=== Testing decrypt_and_verify function ===");
//     match provider
//         .decrypt_and_verify(
//             &encrypted,
//             &recipient_keypair.private_key,
//             &sender_keypair.public_key,
//             None,
//         )
//         .await
//     {
//         Ok(result) => {
//             println!("decrypt_and_verify successful!");
//             println!("Data: {:?}", String::from_utf8_lossy(&result.data));
//             println!("Signature valid: {}", result.signature_valid);
//         }
//         Err(e) => {
//             println!("decrypt_and_verify failed: {}", e);
//         }
//     }
// }