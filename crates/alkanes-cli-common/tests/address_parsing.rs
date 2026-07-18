use bech32;

#[test]
fn test_parse_bech32_address() {
    let addr_str = "bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw";
    let decoded = bech32::decode(addr_str);
    assert!(decoded.is_ok(), "Failed to parse bech32 address: {:?}", decoded.err());
}