// Debug test file - all imports commented out since tests are disabled
// use deezel_rpgp::{
//     composed::{
//         MessageBuilder, ArmorOptions, Message, SecretKeyParamsBuilder,
//         SignedPublicKey, KeyType,
//     },
//     crypto::{hash::HashAlgorithm, sym::SymmetricKeyAlgorithm},
//     types::Password,
//     io::Read,
// };

// #[tokio::test]
// async fn test_reference_pattern() {
//     // Generate keys like the reference
//     let mut rng = rand::thread_rng();
    
//     let key_params = SecretKeyParamsBuilder::default()
//         .key_type(KeyType::Rsa(2048))
//         .can_sign(true)
//         .primary_user_id("test@example.com".into())
//         .preferred_symmetric_algorithms(smallvec::smallvec![SymmetricKeyAlgorithm::AES128])
//         .preferred_hash_algorithms(smallvec::smallvec![HashAlgorithm::Sha256])
//         .build()
//         .unwrap();

//     let secret_key = key_params
//         .generate(&mut rng)
//         .unwrap();

//     let signed_secret_key = secret_key
//         .sign(&mut rng, &Password::empty())
//         .unwrap();

//     let signed_public_key: SignedPublicKey = signed_secret_key.clone().into();

//     // Encrypt and sign following the exact reference pattern
//     let mut builder = MessageBuilder::from_bytes("", "Testing\n")
//         .seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES128);
    
//     builder
//         .sign(&*signed_secret_key, Password::empty(), HashAlgorithm::Sha256)
//         .encrypt_to_key(&mut rng, &signed_public_key)
//         .unwrap();

//     let out = builder.to_armored_string(&mut rng, ArmorOptions::default()).unwrap();

//     println!("Encrypted message length: {}", out.len());
//     println!("First 100 chars: {}", &out[..100.min(out.len())]);

//     // Decrypt and verify following the exact reference pattern
//     let (msg, _) = Message::from_armor(out.as_bytes()).unwrap();
    
//     println!("Message parsed successfully");
    
//     let mut msg = msg.decrypt(&Password::empty(), &signed_secret_key).unwrap();
    
//     println!("Message decrypted successfully");
    
//     // Try to read the data manually instead of using as_data_string()
//     let mut data_vec = Vec::new();
//     let mut temp_buf = [0; 1024];
//     loop {
//         match msg.read(&mut temp_buf) {
//             Ok(0) => break,
//             Ok(n) => {
//                 println!("Read {} bytes", n);
//                 data_vec.extend_from_slice(&temp_buf[..n]);
//             }
//             Err(e) => {
//                 println!("Error reading from message: {:?}", e);
//                 break;
//             }
//         }
//     }
//     let data = String::from_utf8(data_vec).unwrap();
//     println!("Data extracted: '{}'", data);
//     assert_eq!(data, "Testing\n");
    
//     msg.verify(&signed_public_key).unwrap();
//     println!("Signature verified successfully");
// }