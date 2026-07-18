EOF
cp /data/alkanes-rs/reference/deezel/crates/deezel-sys/src/keystore.rs keystore_orig.rs
# Now apply only the essential changes
sed -i 's/deezel_common/alkanes_cli_common/g; s/DeezelError/AlkanesError/g' keystore_orig.rs
sed -i 's/use bip39::{Mnemonic, MnemonicType};/use bip39::Mnemonic;/g' keystore_orig.rs
sed -i 's/Mnemonic::new(MnemonicType::Words24, bip39::Language::English)/Mnemonic::from_entropy(\&rand::random::<[u8; 32]>()).map_err(|e| AlkanesError::Wallet(format!("Failed to generate mnemonic: {e}")))?/g' keystore_orig.rs
sed -i 's/Mnemonic::from_phrase(\([^,]*\), bip39::Language::English)/Mnemonic::parse_in(bip39::Language::English, \1)/g' keystore_orig.rs
sed -i 's/\.phrase()/.to_string()/g' keystore_orig.rs
mv keystore_orig.rs keystore.rs
