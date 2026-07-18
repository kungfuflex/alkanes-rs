extern crate alloc;
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
    format,
};
use core::{fmt, str};
use crate::errors::Result;

use nom::{
    branch::alt,
    bytes::streaming::{tag, take_until1},
    character::streaming::{digit1, line_ending, not_line_ending},
    combinator::{complete, map, map_res, opt, value},
    multi::many0,
    sequence::{delimited, pair, preceded, terminated},
    AsChar, IResult, Parser,
};

/// Armor block types.
///
/// Both OpenPGP (RFC 9580) and OpenSSL PEM armor types are included.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum BlockType {
    /// PGP public key
    PublicKey,
    /// PEM encoded PKCS#1 public key
    PublicKeyPKCS1(PKCS1Type),
    /// PEM encoded PKCS#8 public key
    PublicKeyPKCS8,
    /// Public key OpenSSH
    PublicKeyOpenssh,
    /// PGP private key
    PrivateKey,
    /// PEM encoded PKCS#1 private key
    PrivateKeyPKCS1(PKCS1Type),
    /// PEM encoded PKCS#8 private key
    PrivateKeyPKCS8,
    /// OpenSSH private key
    PrivateKeyOpenssh,
    Message,
    MultiPartMessage(usize, usize),
    Signature,
    // gnupgp extension
    File,
    /// Cleartext Framework message
    CleartextMessage,
    /// Encrypted Mnemonic
    EncryptedMnemonic,
}

impl fmt::Display for BlockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockType::PublicKey => f.write_str("PGP PUBLIC KEY BLOCK"),
            BlockType::PrivateKey => f.write_str("PGP PRIVATE KEY BLOCK"),
            BlockType::EncryptedMnemonic => f.write_str("ENCRYPTED MNEMONIC"),
            BlockType::MultiPartMessage(x, y) => write!(f, "PGP MESSAGE, PART {x}/{y}"),
            BlockType::Message => f.write_str("PGP MESSAGE"),
            BlockType::Signature => f.write_str("PGP SIGNATURE"),
            BlockType::File => f.write_str("PGP ARMORED FILE"),
            BlockType::PublicKeyPKCS1(typ) => write!(f, "{typ} PUBLIC KEY"),
            BlockType::PublicKeyPKCS8 => f.write_str("PUBLIC KEY"),
            BlockType::PublicKeyOpenssh => f.write_str("OPENSSH PUBLIC KEY"),
            BlockType::PrivateKeyPKCS1(typ) => write!(f, "{typ} PRIVATE KEY"),
            BlockType::PrivateKeyPKCS8 => f.write_str("PRIVATE KEY"),
            BlockType::PrivateKeyOpenssh => f.write_str("OPENSSH PRIVATE KEY"),
            BlockType::CleartextMessage => f.write_str("PGP SIGNED MESSAGE"),
        }
    }
}


/// OpenSSL PKCS#1 PEM armor types
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum PKCS1Type {
    RSA,
    DSA,
    EC,
}

impl fmt::Display for PKCS1Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PKCS1Type::RSA => write!(f, "RSA"),
            PKCS1Type::DSA => write!(f, "DSA"),
            PKCS1Type::EC => write!(f, "EC"),
        }
    }
}

/// Armor Headers.
pub type Headers = BTreeMap<String, Vec<String>>;

/// Parses a single ascii armor header separator.
fn armor_header_sep(i: &[u8]) -> IResult<&[u8], &[u8]> {
    tag(&b"-----"[..])(i)
}

#[inline]
fn parse_digit(x: &[u8]) -> Result<usize> {
    let s = str::from_utf8(x).map_err(|_| nom::Err::Failure(nom::error::Error::new(x, nom::error::ErrorKind::Char)))?;
    let digit: usize = s.parse().map_err(|_| nom::Err::Failure(nom::error::Error::new(x, nom::error::ErrorKind::Char)))?;
    Ok(digit)
}

/// Parses the type inside of an ascii armor header.
fn armor_header_type(i: &[u8]) -> IResult<&[u8], BlockType> {
    alt((
        value(BlockType::PublicKey, tag("PGP PUBLIC KEY BLOCK")),
        value(BlockType::PrivateKey, tag("PGP PRIVATE KEY BLOCK")),
        map(
            preceded(
                tag("PGP MESSAGE, PART "),
                pair(
                    map_res(digit1, parse_digit),
                    opt(preceded(tag("/"), map_res(digit1, parse_digit))),
                ),
            ),
            |(x, y)| BlockType::MultiPartMessage(x, y.unwrap_or(0)),
        ),
        value(BlockType::Message, tag("PGP MESSAGE")),
        value(BlockType::Signature, tag("PGP SIGNATURE")),
        value(BlockType::File, tag("PGP ARMORED FILE")),
        value(BlockType::CleartextMessage, tag("PGP SIGNED MESSAGE")),
        value(BlockType::EncryptedMnemonic, tag("ENCRYPTED MNEMONIC")),
        // OpenSSL formats
        // Public Key File PKCS#1
        value(
            BlockType::PublicKeyPKCS1(PKCS1Type::RSA),
            tag("RSA PUBLIC KEY"),
        ),
        // Public Key File PKCS#1
        value(
            BlockType::PublicKeyPKCS1(PKCS1Type::DSA),
            tag("DSA PUBLIC KEY"),
        ),
        // Public Key File PKCS#1
        value(
            BlockType::PublicKeyPKCS1(PKCS1Type::EC),
            tag("EC PUBLIC KEY"),
        ),
        // Public Key File PKCS#8
        value(BlockType::PublicKeyPKCS8, tag("PUBLIC KEY")),
        // OpenSSH Public Key File
        value(BlockType::PublicKeyOpenssh, tag("OPENSSH PUBLIC KEY")),
        // Private Key File PKCS#1
        value(
            BlockType::PrivateKeyPKCS1(PKCS1Type::RSA),
            tag("RSA PRIVATE KEY"),
        ),
        // Private Key File PKCS#1
        value(
            BlockType::PrivateKeyPKCS1(PKCS1Type::DSA),
            tag("DSA PRIVATE KEY"),
        ),
        // Private Key File PKCS#1
        value(
            BlockType::PrivateKeyPKCS1(PKCS1Type::EC),
            tag("EC PRIVATE KEY"),
        ),
        // Private Key File PKCS#8
        value(BlockType::PrivateKeyPKCS8, tag("PRIVATE KEY")),
        // OpenSSH Private Key File
        value(BlockType::PrivateKeyOpenssh, tag("OPENSSH PRIVATE KEY")),
    ))
    .parse(i)
}

/// Parses a single armor header line.
fn armor_header_line(i: &[u8]) -> IResult<&[u8], BlockType> {
    delimited(
        pair(armor_header_sep, tag(&b"BEGIN "[..])),
        armor_header_type,
        pair(armor_header_sep, line_ending),
    )
    .parse(i)
}

/// Parses a single key value pair, for the header.
fn key_value_pair(i: &[u8]) -> IResult<&[u8], (&str, &str)> {
    let (i, key) = map_res(
        alt((
            complete(take_until1(":\r\n")),
            complete(take_until1(":\n")),
            complete(take_until1(": ")),
        )),
        str::from_utf8,
    )
    .parse(i)?;

    // consume the ":"
    let (i, _) = tag(":")(i)?;
    let (i, t) = alt((tag(" "), line_ending)).parse(i)?;

    let (i, value) = if t == b" " {
        let (i, value) = map_res(not_line_ending, str::from_utf8).parse(i)?;
        let (i, _) = line_ending(i)?;
        (i, value)
    } else {
        // empty value
        (i, "")
    };

    Ok((i, (key, value)))
}

/// Parses a list of key value pairs.
fn key_value_pairs(i: &[u8]) -> IResult<&[u8], Vec<(&str, &str)>> {
    many0(complete(key_value_pair)).parse(i)
}

/// Parses the full armor header.
fn armor_headers(i: &[u8]) -> IResult<&[u8], Headers> {
    map(key_value_pairs, |pairs| {
        // merge multiple values with the same name
        let mut out = BTreeMap::<String, Vec<String>>::new();
        for (k, v) in pairs {
            let e = out.entry(k.to_string()).or_default();
            e.push(v.to_string());
        }
        out
    })
    .parse(i)
}

/// Armor Header
pub fn armor_header(i: &[u8]) -> IResult<&[u8], (BlockType, Headers)> {
    let (i, typ) = armor_header_line(i)?;
    let (i, headers) = match typ {
        BlockType::CleartextMessage => armor_headers_hash(i)?,
        _ => armor_headers(i)?,
    };

    Ok((i, (typ, headers)))
}

fn armor_headers_hash(i: &[u8]) -> IResult<&[u8], Headers> {
    let (i, headers) = many0(complete(hash_header_line)).parse(i)?;

    let mut res = BTreeMap::new();
    let headers = headers.into_iter().flatten().collect();
    res.insert("Hash".to_string(), headers);

    Ok((i, res))
}

pub fn alphanumeric1_or_dash<T, E: nom::error::ParseError<T>>(input: T) -> IResult<T, T, E>
where
    T: nom::InputTakeAtPosition,
    T: nom::InputTakeAtPosition,
    <T as nom::InputTakeAtPosition>::Item: nom::AsChar,
{
    input.split_at_position1(
        |item| {
            let i = item.as_char();

            !(i.is_alphanum() || i == '-')
        },
        nom::error::ErrorKind::AlphaNumeric,
    )
}

fn hash_header_line(i: &[u8]) -> IResult<&[u8], Vec<String>> {
    let (i, _) = tag("Hash: ")(i)?;
    let (i, mut values) = many0(map_res(terminated(alphanumeric1_or_dash, tag(",")), |s| {
        str::from_utf8(s).map(|s| s.to_string())
    }))
    .parse(i)?;

    let (i, last_value) = terminated(
        map_res(alphanumeric1_or_dash, |s| {
            str::from_utf8(s).map(|s| s.to_string())
        }),
        line_ending,
    )
    .parse(i)?;
    values.push(last_value);

    Ok((i, values))
}

pub fn decode(i: &[u8]) -> Result<(BlockType, Headers, Vec<u8>)> {
    let (remaining, (typ, headers)) = armor_header(i)?;

    // Skip the blank line after headers
    let remaining = if remaining.starts_with(b"\r\n") {
        &remaining[2..]
    } else if remaining.starts_with(b"\n") {
        &remaining[1..]
    } else {
        remaining
    };

    // Find the footer and extract the base64 content
    let footer_start = if let Some(pos) = find_footer_start(remaining) {
        pos
    } else {
        return Err(crate::errors::Error::from("armor footer not found"));
    };

    let base64_content = &remaining[..footer_start];
    
    // Clean up the base64 content by removing line endings and whitespace
    let cleaned_base64: Vec<u8> = base64_content
        .iter()
        .filter(|&&b| !matches!(b, b'\r' | b'\n' | b' ' | b'\t'))
        .copied()
        .collect();

    // Decode the base64 content directly
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD.decode(&cleaned_base64)
        .map_err(|e| crate::errors::Error::from(format!("base64 decode error: {e}")))?;

    Ok((typ, headers, decoded))
}

// Helper function to find the start of the armor footer
fn find_footer_start(data: &[u8]) -> Option<usize> {
    // Look for patterns like "=XXXX\n-----END" or "\n-----END" or "-----END"
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(b"-----END") {
            return Some(i);
        }
        if data[i] == b'=' {
            // Look for checksum pattern like "=XXXX\n-----END"
            let mut j = i + 1;
            while j < data.len() && j < i + 10 {
                if data[j] == b'\n' || data[j] == b'\r' {
                    if data[j..].starts_with(b"\n-----END") || data[j..].starts_with(b"\r\n-----END") {
                        return Some(i);
                    }
                    break;
                }
                j += 1;
            }
        }
        i += 1;
    }
    None
}
