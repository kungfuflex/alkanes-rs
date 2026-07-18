use deezel_common::pgp_rpgp::RpgpPgpProvider;
use deezel_common::traits::PgpProvider;

#[tokio::test]
async fn debug_encrypt_decrypt_binary() {
    let provider = RpgpPgpProvider::new();
    let keypair = provider
        .generate_keypair("testuser", None)
        .await
        .unwrap();

    let data = b"hello world";
    
    // Test binary encryption first
    let encrypted_binary = provider
        .encrypt(data, &[keypair.public_key.clone()], false)
        .await
        .unwrap();

    println!("Binary encrypted data length: {}", encrypted_binary.len());
    println!("Binary encrypted data (hex): {}", hex::encode(&encrypted_binary));
    println!("First 50 bytes: {:?}", &encrypted_binary[..50.min(encrypted_binary.len())]);

    let decrypted_binary = provider
        .decrypt(&encrypted_binary, &keypair.private_key, None)
        .await
        .unwrap();

    assert_eq!(data.to_vec(), decrypted_binary);
    
    // Test armored encryption
    let encrypted_armored = provider
        .encrypt(data, &[keypair.public_key.clone()], true)
        .await
        .unwrap();

    println!("Armored encrypted data length: {}", encrypted_armored.len());
    println!("Armored encrypted data: {}", String::from_utf8_lossy(&encrypted_armored));

    let decrypted_armored = provider
        .decrypt(&encrypted_armored, &keypair.private_key, None)
        .await
        .unwrap();

    assert_eq!(data.to_vec(), decrypted_armored);
}