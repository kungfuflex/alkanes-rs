// Debug test file - all imports commented out since tests are disabled
// use deezel_common::pgp_rpgp::RpgpPgpProvider;
// use deezel_common::traits::PgpProvider;

// #[tokio::test]
// async fn debug_encrypt_and_sign() {
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
    
//     // Test armored encrypt_and_sign
//     println!("Testing armored encrypt_and_sign...");
//     let encrypted_armored = provider
//         .encrypt_and_sign(
//             data,
//             &[recipient_keypair.public_key.clone()],
//             &sender_keypair.private_key,
//             None,
//             true, // armor = true
//         )
//         .await
//         .unwrap();
    
//     println!("Encrypted armored length: {}", encrypted_armored.len());
//     println!("Encrypted armored data:\n{}", String::from_utf8_lossy(&encrypted_armored));
    
//     // Test binary encrypt_and_sign
//     println!("\nTesting binary encrypt_and_sign...");
//     let encrypted_binary = provider
//         .encrypt_and_sign(
//             data,
//             &[recipient_keypair.public_key.clone()],
//             &sender_keypair.private_key,
//             None,
//             false, // armor = false
//         )
//         .await
//         .unwrap();
    
//     println!("Encrypted binary length: {}", encrypted_binary.len());
//     println!("Encrypted binary (hex): {}", hex::encode(&encrypted_binary));
    
//     // Try to decrypt the armored version
//     println!("\nTrying to decrypt armored version...");
//     match provider
//         .decrypt_and_verify(
//             &encrypted_armored,
//             &recipient_keypair.private_key,
//             &sender_keypair.public_key,
//             None,
//         )
//         .await
//     {
//         Ok(result) => {
//             println!("Decryption successful!");
//             println!("Data: {:?}", String::from_utf8_lossy(&result.data));
//             println!("Signature valid: {}", result.signature_valid);
//         }
//         Err(e) => {
//             println!("Decryption failed: {:?}", e);
//         }
//     }
    
//     // Try to decrypt the binary version
//     println!("\nTrying to decrypt binary version...");
//     match provider
//         .decrypt_and_verify(
//             &encrypted_binary,
//             &recipient_keypair.private_key,
//             &sender_keypair.public_key,
//             None,
//         )
//         .await
//     {
//         Ok(result) => {
//             println!("Decryption successful!");
//             println!("Data: {:?}", String::from_utf8_lossy(&result.data));
//             println!("Signature valid: {}", result.signature_valid);
//         }
//         Err(e) => {
//             println!("Decryption failed: {:?}", e);
//         }
//     }
// }