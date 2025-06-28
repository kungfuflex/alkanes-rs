use {
    super::*,
    bitcoin::{
        blockdata::{opcodes, script},
        ScriptBuf, Transaction,
    },
    std::str,
};

#[derive(Debug, PartialEq, Clone)]
pub struct Inscription {
    body: Option<Vec<u8>>,
    content_type: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq)]
pub enum ParsedInscription {
    None,
    Partial,
    Complete(Inscription),
}

impl Inscription {
    /// Create a new inscription with the given content type and body
    pub fn new(content_type: Option<Vec<u8>>, body: Option<Vec<u8>>) -> Self {
        Self { content_type, body }
    }

    /// Parse inscriptions from a series of Dogecoin transactions
    /// Uses script_sig fields and supports multi-transaction inscriptions
    pub fn from_transactions(txs: Vec<Transaction>) -> ParsedInscription {
        if txs.is_empty() {
            return ParsedInscription::None;
        }

        let mut sig_scripts = Vec::with_capacity(txs.len());
        for tx in &txs {
            if tx.input.is_empty() {
                return ParsedInscription::None;
            }
            sig_scripts.push(tx.input[0].script_sig.clone());
        }
        InscriptionParser::parse(sig_scripts)
    }

    /// Get the inscription body as bytes
    pub fn body(&self) -> Option<&[u8]> {
        Some(self.body.as_ref()?)
    }

    /// Get the inscription body, consuming the inscription
    pub fn into_body(self) -> Option<Vec<u8>> {
        self.body
    }

    /// Get the content length
    pub fn content_length(&self) -> Option<usize> {
        Some(self.body()?.len())
    }

    /// Get the content type as a string
    pub fn content_type(&self) -> Option<&str> {
        str::from_utf8(self.content_type.as_ref()?).ok()
    }

    /// Create a reveal script for this inscription (for creating inscriptions)
    pub fn append_reveal_script(&self, mut builder: script::Builder) -> ScriptBuf {
        use bitcoin::script::PushBytesBuf;
        
        builder = builder
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(PushBytesBuf::try_from(PROTOCOL_ID.to_vec()).unwrap());

        if let Some(content_type) = &self.content_type {
            builder = builder
                .push_slice(PushBytesBuf::try_from(vec![1]).unwrap())
                .push_slice(PushBytesBuf::try_from(content_type.clone()).unwrap());
        }

        if let Some(body) = &self.body {
            builder = builder.push_slice(PushBytesBuf::try_from(vec![]).unwrap());
            for chunk in body.chunks(MAX_PUSH_SIZE) {
                builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec()).unwrap());
            }
        }

        builder.push_opcode(opcodes::all::OP_ENDIF).into_script()
    }
}

struct InscriptionParser;

impl InscriptionParser {
    fn parse(sig_scripts: Vec<ScriptBuf>) -> ParsedInscription {
        if sig_scripts.is_empty() {
            return ParsedInscription::None;
        }

        let sig_script = &sig_scripts[0];

        let mut push_datas_vec = match Self::decode_push_datas(sig_script) {
            Some(push_datas) => push_datas,
            None => return ParsedInscription::None,
        };

        let mut push_datas = push_datas_vec.as_slice();

        // Read protocol identifier
        if push_datas.len() < 3 {
            return ParsedInscription::None;
        }

        let protocol = &push_datas[0];
        if protocol != PROTOCOL_ID {
            return ParsedInscription::None;
        }

        // Read npieces (number of pieces this inscription is split into)
        let mut npieces = match Self::push_data_to_number(&push_datas[1]) {
            Some(n) => n,
            None => return ParsedInscription::None,
        };

        if npieces == 0 {
            return ParsedInscription::None;
        }

        // Read content type
        let content_type = push_datas[2].clone();
        push_datas = &push_datas[3..];

        // Read body across potentially multiple transactions
        let mut body = vec![];
        let mut sig_scripts = sig_scripts.as_slice();

        // Loop over transactions
        loop {
            // Loop over chunks within current transaction
            loop {
                if npieces == 0 {
                    let inscription = Inscription {
                        content_type: Some(content_type),
                        body: Some(body),
                    };
                    return ParsedInscription::Complete(inscription);
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

            if sig_scripts.len() <= 1 {
                return ParsedInscription::Partial;
            }

            sig_scripts = &sig_scripts[1..];

            push_datas_vec = match Self::decode_push_datas(&sig_scripts[0]) {
                Some(push_datas) => push_datas,
                None => return ParsedInscription::None,
            };

            if push_datas_vec.len() < 2 {
                return ParsedInscription::None;
            }

            let next = match Self::push_data_to_number(&push_datas_vec[0]) {
                Some(n) => n,
                None => return ParsedInscription::None,
            };

            if next != npieces - 1 {
                return ParsedInscription::None;
            }

            push_datas = push_datas_vec.as_slice();
        }
    }

    fn decode_push_datas(script: &ScriptBuf) -> Option<Vec<Vec<u8>>> {
        let mut bytes = script.as_bytes();
        let mut push_datas = vec![];

        while !bytes.is_empty() {
            // OP_0
            if bytes[0] == 0 {
                push_datas.push(vec![]);
                bytes = &bytes[1..];
                continue;
            }

            // OP_1 - OP_16
            if bytes[0] >= 81 && bytes[0] <= 96 {
                push_datas.push(vec![bytes[0] - 80]);
                bytes = &bytes[1..];
                continue;
            }

            // OP_PUSH 1-75
            if bytes[0] >= 1 && bytes[0] <= 75 {
                let len = bytes[0] as usize;
                if bytes.len() < 1 + len {
                    return None;
                }
                push_datas.push(bytes[1..1 + len].to_vec());
                bytes = &bytes[1 + len..];
                continue;
            }

            // OP_PUSHDATA1
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

            // OP_PUSHDATA2
            if bytes[0] == 77 {
                if bytes.len() < 3 {
                    return None;
                }
                let len = ((bytes[2] as usize) << 8) + (bytes[1] as usize);
                if bytes.len() < 3 + len {
                    return None;
                }
                push_datas.push(bytes[3..3 + len].to_vec());
                bytes = &bytes[3 + len..];
                continue;
            }

            // OP_PUSHDATA4
            if bytes[0] == 78 {
                if bytes.len() < 5 {
                    return None;
                }
                let len = ((bytes[4] as usize) << 24)
                    + ((bytes[3] as usize) << 16)
                    + ((bytes[2] as usize) << 8)
                    + (bytes[1] as usize);
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
        if data.is_empty() {
            return Some(0);
        }

        if data.len() > 8 {
            return None;
        }

        let mut n: u64 = 0;
        let mut m: u64 = 0;

        for &byte in data {
            n += (byte as u64) << m;
            m += 8;
        }

        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{TxIn, Witness, OutPoint, Sequence};

    fn inscription(content_type: &str, body: &str) -> Inscription {
        Inscription::new(
            Some(content_type.as_bytes().to_vec()),
            Some(body.as_bytes().to_vec()),
        )
    }

    #[test]
    fn test_empty() {
        assert_eq!(
            InscriptionParser::parse(vec![ScriptBuf::new()]),
            ParsedInscription::None
        );
    }

    #[test]
    fn test_valid() {
        let mut script: Vec<&[u8]> = Vec::new();
        script.push(&[3]);
        script.push(b"ord");
        script.push(&[81]); // OP_1
        script.push(&[24]);
        script.push(b"text/plain;charset=utf-8");
        script.push(&[0]);
        script.push(&[4]);
        script.push(b"woof");
        
        assert_eq!(
            InscriptionParser::parse(vec![ScriptBuf::from(script.concat())]),
            ParsedInscription::Complete(inscription("text/plain;charset=utf-8", "woof"))
        );
    }

    #[test]
    fn test_valid_multipart() {
        let mut script: Vec<&[u8]> = Vec::new();
        script.push(&[3]);
        script.push(b"ord");
        script.push(&[82]); // OP_2
        script.push(&[24]);
        script.push(b"text/plain;charset=utf-8");
        script.push(&[81]); // countdown = 1
        script.push(&[4]);
        script.push(b"woof");
        script.push(&[0]); // countdown = 0
        script.push(&[5]);
        script.push(b" woof");
        
        assert_eq!(
            InscriptionParser::parse(vec![ScriptBuf::from(script.concat())]),
            ParsedInscription::Complete(inscription("text/plain;charset=utf-8", "woof woof"))
        );
    }

    #[test]
    fn test_wrong_protocol() {
        let mut script: Vec<&[u8]> = Vec::new();
        script.push(&[3]);
        script.push(b"dog");
        script.push(&[81]);
        script.push(&[24]);
        script.push(b"text/plain;charset=utf-8");
        script.push(&[0]);
        script.push(&[4]);
        script.push(b"woof");
        
        assert_eq!(
            InscriptionParser::parse(vec![ScriptBuf::from(script.concat())]),
            ParsedInscription::None
        );
    }
}