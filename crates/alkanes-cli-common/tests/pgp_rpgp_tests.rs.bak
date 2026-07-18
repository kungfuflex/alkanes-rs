use deezel_common::pgp_rpgp::RpgpPgpProvider;
use deezel_common::traits::PgpProvider;

#[tokio::test]
async fn test_generate_keypair() {
    let provider = RpgpPgpProvider::new();
    let keypair = provider
        .generate_keypair("testuser", Some("password"))
        .await
        .unwrap();

    assert!(!keypair.public_key.is_private);
    assert!(keypair.private_key.is_private);
    assert_eq!(keypair.public_key.fingerprint, keypair.private_key.fingerprint);
    assert_eq!(keypair.public_key.key_id, keypair.private_key.key_id);
}

#[tokio::test]
async fn test_import_export_key() {
    let provider = RpgpPgpProvider::new();
    let keypair = provider
        .generate_keypair("testuser", Some("password"))
        .await
        .unwrap();

    let exported_public_key = provider
        .export_key(&keypair.public_key, false)
        .await
        .unwrap();
    let imported_public_key = provider.import_key(&exported_public_key).await.unwrap();

    assert_eq!(keypair.public_key.fingerprint, imported_public_key.fingerprint);

    let exported_private_key = provider
        .export_key(&keypair.private_key, true)
        .await
        .unwrap();
    let imported_private_key = provider.import_key(&exported_private_key).await.unwrap();

    assert_eq!(keypair.private_key.fingerprint, imported_private_key.fingerprint);
}

#[tokio::test]
async fn test_encrypt_decrypt() {
    let provider = RpgpPgpProvider::new();
    let keypair = provider
        .generate_keypair("testuser", None)
        .await
        .unwrap();

    let data = b"hello world";
    let encrypted = provider
        .encrypt(data, &[keypair.public_key.clone()], true)
        .await
        .unwrap();

    println!("Encrypted data length: {}", encrypted.len());
    println!("Encrypted data (hex): {}", hex::encode(&encrypted));
    println!("First 20 bytes: {:?}", &encrypted[..20.min(encrypted.len())]);

    let decrypted = provider
        .decrypt(&encrypted, &keypair.private_key, None)
        .await
        .unwrap();

    assert_eq!(data.to_vec(), decrypted);
}

#[tokio::test]
async fn test_sign_verify() {
    let provider = RpgpPgpProvider::new();
    let keypair = provider
        .generate_keypair("testuser", None)
        .await
        .unwrap();

    let data = b"hello world";
    let signature = provider
        .sign(data, &keypair.private_key, None, true)
        .await
        .unwrap();

    let verified = provider
        .verify(data, &signature, &keypair.public_key)
        .await
        .unwrap();

    assert!(verified);
}

// #[tokio::test]
// async fn test_encrypt_and_sign_decrypt_and_verify() {
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
//     let encrypted = provider
//         .encrypt_and_sign(
//             data,
//             &[recipient_keypair.public_key.clone()],
//             &sender_keypair.private_key,
//             None,
//             true,
//         )
//         .await
//         .unwrap();

//     let decrypted = provider
//         .decrypt_and_verify(
//             &encrypted,
//             &recipient_keypair.private_key,
//             &sender_keypair.public_key,
//             None,
//         )
//         .await
//         .unwrap();

//     assert_eq!(data.to_vec(), decrypted.data);
//     assert!(decrypted.signature_valid);
//     assert_eq!(
//         decrypted.signer_key_id,
//         Some(sender_keypair.public_key.key_id)
//     );
// }