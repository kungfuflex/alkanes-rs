use deezel_common::pgp_rpgp::RpgpPgpProvider;
use deezel_common::traits::PgpProvider;

#[tokio::test]
async fn debug_signed_message_reading() {
    let provider = RpgpPgpProvider::new();
    let keypair = provider
        .generate_keypair("testuser", None)
        .await
        .unwrap();

    let data = b"hello world";
    
    // Test just signing (not encrypt+sign)
    println!("=== Testing sign only ===");
    let signature = provider
        .sign(data, &keypair.private_key, None, true)
        .await
        .unwrap();
    
    let verified = provider
        .verify(data, &signature, &keypair.public_key)
        .await
        .unwrap();
    
    println!("Sign/verify works: {}", verified);
    
    // Test just encryption (not encrypt+sign)
    println!("=== Testing encrypt only ===");
    let encrypted = provider
        .encrypt(data, &[keypair.public_key.clone()], true)
        .await
        .unwrap();
    
    let decrypted = provider
        .decrypt(&encrypted, &keypair.private_key, None)
        .await
        .unwrap();
    
    println!("Encrypt/decrypt works: {:?}", String::from_utf8_lossy(&decrypted));
    
    // Now test the problematic case: what happens when we manually create a signed message
    // and try to read from it?
    println!("=== Testing manual signed message creation ===");
    
    // Create a simple literal message first
    let literal_data = b"test data";
    let literal_message = deezel_rpgp::composed::MessageBuilder::from_bytes("test", literal_data.to_vec())
        .to_vec(&mut rand::thread_rng())
        .unwrap();
    
    println!("Created literal message: {} bytes", literal_message.len());
    
    // Parse it back
    let mut parsed_literal = deezel_rpgp::composed::Message::from_bytes(&literal_message).unwrap();
    let literal_data_back = parsed_literal.as_data_vec().unwrap();
    println!("Literal message data: {:?}", String::from_utf8_lossy(&literal_data_back));
    
    // Now try to create a signed message manually and see if we can read from it
    println!("=== Testing signed message structure ===");
    
    // This should help us understand if the issue is in the signing process or the reading process
}