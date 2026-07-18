use deezel_common::pgp_rpgp::RpgpPgpProvider;
use deezel_common::traits::PgpProvider;

#[tokio::test]
async fn debug_armor_parsing() {
    let provider = RpgpPgpProvider::new();

    // Generate a keypair
    let keypair = provider.generate_keypair("testuser", Some("password")).await.expect("Failed to generate keypair");
    
    // Export the public key
    let exported_key = provider.export_key(&keypair.public_key, false).await.expect("Failed to export key");
    println!("Exported key:\n{}", exported_key);
    
    // Try to import it back - this should fail with "no matching packet found"
    match provider.import_key(&exported_key).await {
        Ok(imported_key) => {
            println!("Successfully imported key: {:?}", imported_key.fingerprint);
        }
        Err(e) => {
            println!("Failed to import key: {:?}", e);
        }
    }
}