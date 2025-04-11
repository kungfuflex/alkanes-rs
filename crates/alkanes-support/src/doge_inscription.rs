use {
  bitcoin::{
    blockdata::{opcodes, script},
    ScriptBuf, Transaction, Witness,
  },
  std::str,
  std::path::Path,
  bitcoin::script::Builder,
  std::fs,
  anyhow::{anyhow, bail, Context, Result},
};

// Protocol ID for inscriptions
pub const PROTOCOL_ID: [u8; 3] = *b"ord";

// Media type enum
#[derive(Debug, PartialEq, Eq)]
pub enum Media {
    Unknown,
    Wasm,
}

impl Media {
    pub fn content_type_for_path(path: &Path) -> Result<String> {
        if path.extension().and_then(|ext| ext.to_str()) == Some("wasm") {
            Ok(WASM_CONTENT_TYPE.to_string())
        } else {
            Ok("application/octet-stream".to_string())
        }
    }
}

impl std::str::FromStr for Media {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            WASM_CONTENT_TYPE => Ok(Media::Wasm),
            _ => Ok(Media::Unknown),
        }
    }
}

// BIN protocol ID for WASM files
pub const BIN_PROTOCOL_ID: [u8; 3] = *b"BIN";

// Media type for WASM files
pub const WASM_CONTENT_TYPE: &str = "application/wasm";

#[derive(Debug, PartialEq, Clone)]
pub struct DogeInscription {
  body: Option<Vec<u8>>,
  content_type: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq)]
pub enum ParsedDogeInscription {
  None,
  Partial,
  Complete(DogeInscription),
}

impl DogeInscription {
  pub fn new(content_type: Option<Vec<u8>>, body: Option<Vec<u8>>) -> Self {
    Self { content_type, body }
  }

  pub fn from_transactions(txs: Vec<Transaction>) -> ParsedDogeInscription {
    let mut sig_scripts = Vec::with_capacity(txs.len());
    for i in 0..txs.len() {
      if txs[i].input.is_empty() {
        return ParsedDogeInscription::None;
      }
      sig_scripts.push(txs[i].input[0].script_sig.clone());
    }
    DogeInscriptionParser::parse(sig_scripts)
  }

  pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();

    let body = fs::read(path).with_context(|| format!("io error reading {}", path.display()))?;

    // No size limit check for now
    
    let content_type = Media::content_type_for_path(path)?;

    Ok(Self {
      body: Some(body),
      content_type: Some(content_type.into_bytes()),
    })
  }

  fn append_reveal_script_to_builder(&self, mut builder: script::Builder) -> script::Builder {
    // Start with OP_FALSE OP_IF
    builder = builder
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF);
    
    // Push protocol ID
    for &byte in PROTOCOL_ID.iter() {
      builder = builder.push_int(byte as i64);
    }

    if let Some(content_type) = &self.content_type {
      // Push content type tag (1)
      builder = builder.push_int(1);
      
      // Push content type
      for &byte in content_type.iter() {
        builder = builder.push_int(byte as i64);
      }
    }

    if let Some(body) = &self.body {
      // Push empty separator
      builder = builder.push_int(0);
      
      // Push body
      for &byte in body.iter() {
        builder = builder.push_int(byte as i64);
      }
    }

    // End with OP_ENDIF
    builder.push_opcode(opcodes::all::OP_ENDIF)
  }
  
  // Create a BIN inscription for WASM files
  fn append_bin_reveal_script_to_builder(&self, mut builder: script::Builder) -> script::Builder {
    // Start with OP_FALSE OP_IF
    builder = builder
      .push_opcode(opcodes::OP_FALSE)
      .push_opcode(opcodes::all::OP_IF);
    
    // Push BIN protocol ID
    for &byte in BIN_PROTOCOL_ID.iter() {
      builder = builder.push_int(byte as i64);
    }

    if let Some(body) = &self.body {
      // For BIN protocol, we don't need content type tag
      // Push body directly
      for &byte in body.iter() {
        builder = builder.push_int(byte as i64);
      }
    }

    // End with OP_ENDIF
    builder.push_opcode(opcodes::all::OP_ENDIF)
  }

  pub fn append_reveal_script(&self, builder: script::Builder) -> ScriptBuf {
    self.append_reveal_script_to_builder(builder).into_script()
  }

  pub fn media(&self) -> Media {
    if self.body.is_none() {
      return Media::Unknown;
    }

    let Some(content_type) = self.content_type() else {
      return Media::Unknown;
    };

    content_type.parse().unwrap_or(Media::Unknown)
  }

  pub fn body(&self) -> Option<&[u8]> {
    Some(self.body.as_ref()?)
  }

  pub fn into_body(self) -> Option<Vec<u8>> {
    self.body
  }

  pub fn content_length(&self) -> Option<usize> {
    Some(self.body()?.len())
  }

  pub fn content_type(&self) -> Option<&str> {
    str::from_utf8(self.content_type.as_ref()?).ok()
  }

  pub fn to_witness(&self) -> Witness {
    let builder = script::Builder::new();

    let script = self.append_reveal_script(builder);

    let mut witness = Witness::new();

    witness.push(script);
    witness.push([]);

    witness
  }
  pub fn to_gzipped_witness(&self) -> Witness {
    let builder = script::Builder::new();

    // If content type is WASM, use BIN protocol
    let script = if self.content_type() == Some(WASM_CONTENT_TYPE) {
      self.append_bin_reveal_script_to_builder(builder).into_script()
    } else {
      self.append_reveal_script(builder)
    };

    let mut witness = Witness::new();
    witness.push(script);
    witness.push([]);
    witness
  }
  
  // Extract WASM content from a DogeInscription
  pub fn extract_wasm(&self) -> Option<Vec<u8>> {
    if self.content_type() == Some(WASM_CONTENT_TYPE) {
      self.body.clone()
    } else {
      None
    }
  }
  
  // Create a new DogeInscription from a WASM file
  pub fn from_wasm(wasm_data: Vec<u8>) -> Self {
    Self {
      body: Some(wasm_data),
      content_type: Some(WASM_CONTENT_TYPE.as_bytes().to_vec()),
    }
  }
}

struct DogeInscriptionParser;

impl DogeInscriptionParser {
  fn parse(sig_scripts: Vec<ScriptBuf>) -> ParsedDogeInscription {
    if sig_scripts.is_empty() {
      return ParsedDogeInscription::None;
    }
    
    let sig_script = &sig_scripts[0];

    let mut push_datas_vec = match Self::decode_push_datas(sig_script) {
      Some(push_datas) => push_datas,
      None => return ParsedDogeInscription::None,
    };

    let mut push_datas = push_datas_vec.as_slice();

    // read protocol

    if push_datas.len() < 3 {
      return ParsedDogeInscription::None;
    }

    let protocol = &push_datas[0];

    // Accept both ORD and BIN protocol IDs
    if protocol != &PROTOCOL_ID && protocol != &BIN_PROTOCOL_ID {
      return ParsedDogeInscription::None;
    }
    
    // If it's a BIN protocol, handle differently
    let is_bin_protocol = protocol == &BIN_PROTOCOL_ID;

    // read npieces

    let mut npieces = match Self::push_data_to_number(&push_datas[1]) {
      Some(n) => n,
      None => return ParsedDogeInscription::None,
    };

    if npieces == 0 {
      return ParsedDogeInscription::None;
    }

    // read content type
    let content_type = if is_bin_protocol {
      // For BIN protocol, assume it's WASM
      WASM_CONTENT_TYPE.as_bytes().to_vec()
    } else {
      push_datas[2].clone()
    };

    // Skip content type for standard protocol
    push_datas = if is_bin_protocol {
      &push_datas[2..]
    } else {
      &push_datas[3..]
    };

    // read body

    let mut body = vec![];

    // Convert to a vector we can iterate through
    let mut remaining_scripts = sig_scripts.clone();

    // loop over transactions
    loop {
      // loop over chunks
      loop {
        if npieces == 0 {
          let inscription = DogeInscription {
            content_type: Some(content_type),
            body: Some(body),
          };

          return ParsedDogeInscription::Complete(inscription);
        }

        if push_datas.len() < 2 {
          break;
        }

        let next = match Self::push_data_to_number(&push_datas[0]) {
          Some(n) => n,
          None => break,
        };

        if next != npieces - 1 {
          break;
        }

        body.append(&mut push_datas[1].clone());

        push_datas = &push_datas[2..];
        npieces -= 1;
      }

      if remaining_scripts.len() <= 1 {
        return ParsedDogeInscription::Partial;
      }

      remaining_scripts.remove(0);
      
      if remaining_scripts.is_empty() {
        return ParsedDogeInscription::Partial;
      }

      push_datas_vec = match Self::decode_push_datas(&remaining_scripts[0]) {
        Some(push_datas) => push_datas,
        None => return ParsedDogeInscription::None,
      };

      if push_datas_vec.len() < 2 {
        return ParsedDogeInscription::None;
      }

      let next = match Self::push_data_to_number(&push_datas_vec[0]) {
        Some(n) => n,
        None => return ParsedDogeInscription::None,
      };

      if next != npieces - 1 {
        return ParsedDogeInscription::None;
      }

      push_datas = push_datas_vec.as_slice();
    }
  }

  fn decode_push_datas(script: &ScriptBuf) -> Option<Vec<Vec<u8>>> {
    let mut bytes = script.as_bytes();
    let mut push_datas = vec![];

    while !bytes.is_empty() {
      // op_0
      if bytes[0] == 0 {
        push_datas.push(vec![]);
        bytes = &bytes[1..];
        continue;
      }

      // op_1 - op_16
      if bytes[0] >= 81 && bytes[0] <= 96 {
        push_datas.push(vec![bytes[0] - 80]);
        bytes = &bytes[1..];
        continue;
      }

      // op_push 1-75
      if bytes[0] >= 1 && bytes[0] <= 75 {
        let len = bytes[0] as usize;
        if bytes.len() < 1 + len {
          return None;
        }
        push_datas.push(bytes[1..1 + len].to_vec());
        bytes = &bytes[1 + len..];
        continue;
      }

      // op_pushdata1
      if bytes[0] == 76 {
        if bytes.len() < 2 {
          return None;
        }
        let len = bytes[1] as usize;
        if bytes.len() < 2 + len {
          return None;
        }
        push_datas.push(bytes[2..2 + len].to_vec());
        bytes = &bytes[2 + len..];
        continue;
      }

      // op_pushdata2
      if bytes[0] == 77 {
        if bytes.len() < 3 {
          return None;
        }
        let len = ((bytes[1] as usize) << 8) + ((bytes[0] as usize) << 0);
        if bytes.len() < 3 + len {
          return None;
        }
        push_datas.push(bytes[3..3 + len].to_vec());
        bytes = &bytes[3 + len..];
        continue;
      }

      // op_pushdata4
      if bytes[0] == 78 {
        if bytes.len() < 5 {
          return None;
        }
        let len = ((bytes[3] as usize) << 24)
          + ((bytes[2] as usize) << 16)
          + ((bytes[1] as usize) << 8)
          + ((bytes[0] as usize) << 0);
        if bytes.len() < 5 + len {
          return None;
        }
        push_datas.push(bytes[5..5 + len].to_vec());
        bytes = &bytes[5 + len..];
        continue;
      }

      return None;
    }

    Some(push_datas)
  }

  fn push_data_to_number(data: &[u8]) -> Option<u64> {
    if data.len() == 0 {
      return Some(0);
    }

    if data.len() > 8 {
      return None;
    }

    let mut n: u64 = 0;
    let mut m: u64 = 0;

    for i in 0..data.len() {
      n += (data[i] as u64) << m;
      m += 8;
    }

    return Some(n);
  }
}
