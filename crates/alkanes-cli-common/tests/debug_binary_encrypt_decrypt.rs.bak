use deezel_common::pgp_rpgp::RpgpPgpProvider;
use deezel_common::traits::PgpProvider;

#[tokio::test]
async fn test_binary_encrypt_decrypt() {
    let provider = RpgpPgpProvider::new();
    let keypair = provider
        .generate_keypair("testuser", None)
        .await
        .unwrap();

    let data = b"hello world";
    
    // Test binary encryption (armor=false)
    let encrypted = provider
        .encrypt(data, &[keypair.public_key.clone()], false)
        .await
        .unwrap();

    println!("Binary encrypted data length: {}", encrypted.len());
    println!("Binary encrypted data (hex): {}", hex::encode(&encrypted));
    println!("First 20 bytes: {:?}", &encrypted[..20.min(encrypted.len())]);

    let decrypted = provider
        .decrypt(&encrypted, &keypair.private_key, None)
        .await
        .unwrap();

    assert_eq!(data.to_vec(), decrypted);
    println!("Binary encrypt/decrypt test passed!");
}