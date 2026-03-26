//! Host-side WASM module parser.
//!
//! Parses a WASM binary to extract the information needed to build
//! GPU input buffers: import count, function table (code offsets),
//! and the entry point (_start or execute function).
//!
//! This runs on the host (native), NOT inside the WASM VM.

use anyhow::{anyhow, Result};

/// Parsed WASM module information needed for GPU dispatch.
#[derive(Debug, Clone)]
pub struct WasmModuleInfo {
    /// Number of imported functions (host functions come first in the index space)
    pub import_count: u32,
    /// Function table: (code_byte_offset, local_count) for each non-imported function.
    /// Index 0 here corresponds to function index `import_count` in WASM.
    pub functions: Vec<FuncEntry>,
    /// Byte offset of the entry function's code body in the bytecode.
    /// This is the `_start` or designated entry point.
    pub entry_pc: u32,
    /// Raw code section bytes (all function bodies concatenated).
    /// The GPU shader reads bytecode from this.
    pub code_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct FuncEntry {
    /// Byte offset within `code_bytes` where this function's body starts
    /// (after the local declarations).
    pub code_offset: u32,
    /// Number of local variable slots for this function.
    pub local_count: u32,
}

/// WASM section IDs
const SEC_TYPE: u8 = 1;
const SEC_IMPORT: u8 = 2;
const SEC_FUNCTION: u8 = 3;
const SEC_EXPORT: u8 = 7;
const SEC_CODE: u8 = 10;

/// Parse a WASM binary and extract module info for GPU dispatch.
pub fn parse_wasm_module(wasm: &[u8]) -> Result<WasmModuleInfo> {
    let mut pos: usize = 0;

    // Validate magic number and version
    if wasm.len() < 8 {
        return Err(anyhow!("WASM too short"));
    }
    if &wasm[0..4] != b"\0asm" {
        return Err(anyhow!("invalid WASM magic"));
    }
    if &wasm[4..8] != &[1, 0, 0, 0] {
        return Err(anyhow!("unsupported WASM version"));
    }
    pos = 8;

    let mut import_func_count: u32 = 0;
    let mut func_type_indices: Vec<u32> = Vec::new();
    let mut export_start: Option<u32> = None; // function index of _start export
    let mut code_section_start: usize = 0;
    let mut code_section_len: usize = 0;
    let mut functions: Vec<FuncEntry> = Vec::new();

    // Parse sections
    while pos < wasm.len() {
        let section_id = wasm[pos];
        pos += 1;
        let (section_len, bytes_read) = read_leb128_u32(&wasm[pos..])?;
        pos += bytes_read;
        let section_end = pos + section_len as usize;

        match section_id {
            SEC_IMPORT => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    // module name
                    let (mlen, br) = read_leb128_u32(&wasm[p..])?;
                    p += br + mlen as usize;
                    // field name
                    let (flen, br) = read_leb128_u32(&wasm[p..])?;
                    p += br + flen as usize;
                    // import kind
                    let kind = wasm[p];
                    p += 1;
                    match kind {
                        0x00 => {
                            // function import — skip type index
                            let (_, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            import_func_count += 1;
                        }
                        0x01 => {
                            // table import
                            p += 1; // element type
                            let (flags, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            let (_, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            if flags & 1 != 0 {
                                let (_, br) = read_leb128_u32(&wasm[p..])?;
                                p += br;
                            }
                        }
                        0x02 => {
                            // memory import
                            let (flags, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            let (_, br) = read_leb128_u32(&wasm[p..])?;
                            p += br;
                            if flags & 1 != 0 {
                                let (_, br) = read_leb128_u32(&wasm[p..])?;
                                p += br;
                            }
                        }
                        0x03 => {
                            // global import
                            p += 1; // value type
                            p += 1; // mutability
                        }
                        _ => {}
                    }
                }
            }
            SEC_FUNCTION => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    let (type_idx, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    func_type_indices.push(type_idx);
                }
            }
            SEC_EXPORT => {
                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;
                for _ in 0..count {
                    // name
                    let (nlen, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    let name = &wasm[p..p + nlen as usize];
                    p += nlen as usize;
                    // kind
                    let kind = wasm[p];
                    p += 1;
                    // index
                    let (idx, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;

                    if kind == 0x00 {
                        // Function export
                        if name == b"_start" || name == b"execute" {
                            export_start = Some(idx);
                        }
                    }
                }
            }
            SEC_CODE => {
                code_section_start = pos;
                code_section_len = section_len as usize;

                let (count, br) = read_leb128_u32(&wasm[pos..])?;
                let mut p = pos + br;

                // Parse each function body
                for _ in 0..count {
                    let (body_len, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    let body_start = p;

                    // Parse locals
                    let (local_decl_count, br) = read_leb128_u32(&wasm[p..])?;
                    p += br;
                    let mut total_locals: u32 = 0;
                    for _ in 0..local_decl_count {
                        let (count, br) = read_leb128_u32(&wasm[p..])?;
                        p += br;
                        p += 1; // value type
                        total_locals += count;
                    }

                    // Code starts here (after locals)
                    let code_offset = (p - code_section_start) as u32;
                    functions.push(FuncEntry {
                        code_offset,
                        local_count: total_locals,
                    });

                    // Skip to end of body
                    p = body_start + body_len as usize;
                }
            }
            _ => {
                // Skip unknown sections
            }
        }

        pos = section_end;
    }

    // Extract code section bytes
    let code_bytes = if code_section_len > 0 {
        wasm[code_section_start..code_section_start + code_section_len].to_vec()
    } else {
        Vec::new()
    };

    // Determine entry point
    let entry_pc = if let Some(export_idx) = export_start {
        if export_idx >= import_func_count {
            let internal_idx = (export_idx - import_func_count) as usize;
            if internal_idx < functions.len() {
                functions[internal_idx].code_offset
            } else {
                0
            }
        } else {
            // Exported function is an import — shouldn't happen for _start
            0
        }
    } else {
        // No _start or execute export — use first function
        if !functions.is_empty() {
            functions[0].code_offset
        } else {
            0
        }
    };

    Ok(WasmModuleInfo {
        import_count: import_func_count,
        functions,
        entry_pc,
        code_bytes,
    })
}

/// Read a LEB128-encoded u32 from a byte slice.
/// Returns (value, bytes_consumed).
fn read_leb128_u32(data: &[u8]) -> Result<(u32, usize)> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    for (i, &byte) in data.iter().enumerate().take(5) {
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err(anyhow!("LEB128 overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid WASM module with one function that does i32.const 42; end
    fn minimal_wasm() -> Vec<u8> {
        let mut w = Vec::new();

        // Magic + version
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        // Type section: one function type () -> ()
        w.push(SEC_TYPE);
        w.extend_from_slice(&[4]); // section length
        w.push(1); // 1 type
        w.push(0x60); // func type
        w.push(0); // 0 params
        w.push(0); // 0 results

        // Function section: one function, type index 0
        w.push(SEC_FUNCTION);
        w.extend_from_slice(&[2]); // section length
        w.push(1); // 1 function
        w.push(0); // type index 0

        // Export section: export function 0 as "_start"
        w.push(SEC_EXPORT);
        let export_name = b"_start";
        let export_section_len = 1 + 1 + export_name.len() + 1 + 1;
        w.push(export_section_len as u8);
        w.push(1); // 1 export
        w.push(export_name.len() as u8);
        w.extend_from_slice(export_name);
        w.push(0x00); // function export
        w.push(0); // function index 0

        // Code section: one function body
        let body = vec![
            0x00, // 0 local declarations
            0x41, 0x2A, // i32.const 42
            0x1A, // drop
            0x0B, // end
        ];
        let code_section_body_len = 1 + 1 + body.len(); // count(1) + body_len_leb(1) + body
        w.push(SEC_CODE);
        w.push(code_section_body_len as u8);
        w.push(1); // 1 function body
        w.push(body.len() as u8); // body length
        w.extend_from_slice(&body);

        w
    }

    #[test]
    fn test_parse_minimal_wasm() {
        let wasm = minimal_wasm();
        let info = parse_wasm_module(&wasm).unwrap();

        assert_eq!(info.import_count, 0);
        assert_eq!(info.functions.len(), 1);
        assert!(info.entry_pc > 0, "entry_pc should point to code");
        assert!(!info.code_bytes.is_empty());
    }

    /// WASM with 3 imported functions and 2 internal functions
    fn wasm_with_imports() -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"\0asm");
        w.extend_from_slice(&[1, 0, 0, 0]);

        // Helper: write a section (id + LEB128 length + body)
        fn write_section(w: &mut Vec<u8>, id: u8, body: &[u8]) {
            w.push(id);
            // LEB128 encode body length
            let mut len = body.len() as u32;
            loop {
                let mut byte = (len & 0x7F) as u8;
                len >>= 7;
                if len != 0 { byte |= 0x80; }
                w.push(byte);
                if len == 0 { break; }
            }
            w.extend_from_slice(body);
        }

        // Type section: () -> () and () -> (i32)
        write_section(&mut w, SEC_TYPE, &[
            2,    // 2 types
            0x60, 0, 0,             // type 0: () -> ()
            0x60, 0, 1, 0x7F,      // type 1: () -> (i32)
        ]);

        // Import section: 3 function imports from "env"
        let mut import_body = Vec::new();
        import_body.push(3); // 3 imports
        for (name, type_idx) in [("a", 0u8), ("b", 0u8), ("c", 1u8)] {
            import_body.push(3); // "env" length
            import_body.extend_from_slice(b"env");
            import_body.push(name.len() as u8);
            import_body.extend_from_slice(name.as_bytes());
            import_body.push(0x00); // function import
            import_body.push(type_idx);
        }
        write_section(&mut w, SEC_IMPORT, &import_body);

        // Function section: 2 internal functions
        write_section(&mut w, SEC_FUNCTION, &[2, 0, 1]);

        // Export: function 3 (import_count=3 + internal 0) as "_start"
        let mut export_body = Vec::new();
        export_body.push(1); // 1 export
        export_body.push(6); // name length
        export_body.extend_from_slice(b"_start");
        export_body.push(0x00); // function export
        export_body.push(3);   // function index 3
        write_section(&mut w, SEC_EXPORT, &export_body);

        // Code section: 2 function bodies
        let body1: Vec<u8> = vec![0x00, 0x0B]; // 0 locals, end
        let body2: Vec<u8> = vec![0x00, 0x41, 0x2A, 0x0B]; // 0 locals, i32.const 42, end
        let mut code_body = Vec::new();
        code_body.push(2); // 2 function bodies
        code_body.push(body1.len() as u8);
        code_body.extend_from_slice(&body1);
        code_body.push(body2.len() as u8);
        code_body.extend_from_slice(&body2);
        write_section(&mut w, SEC_CODE, &code_body);

        w
    }

    #[test]
    fn test_parse_wasm_with_imports() {
        let wasm = wasm_with_imports();
        let info = parse_wasm_module(&wasm).unwrap();

        assert_eq!(info.import_count, 3);
        assert_eq!(info.functions.len(), 2);
        assert!(info.entry_pc > 0);
    }

    #[test]
    fn test_function_table_for_gpu() {
        let wasm = wasm_with_imports();
        let info = parse_wasm_module(&wasm).unwrap();

        // Build the function table as GPU expects it
        let func_table: Vec<(u32, u32)> = info
            .functions
            .iter()
            .map(|f| (f.code_offset, f.local_count))
            .collect();

        assert_eq!(func_table.len(), 2);
        // Both functions have 0 locals
        assert_eq!(func_table[0].1, 0);
        assert_eq!(func_table[1].1, 0);
        // Second function should have a later offset than first
        assert!(func_table[1].0 > func_table[0].0);
    }
}
