//! wasm reverser + section diff. Ported from the alkane-verifier (fingerprint.rs /
//! wasmsec.rs / diff.rs) into the CLI so the workbench is self-contained. Hand-rolled
//! section walker (no wasmparser) — we control exactly what's parsed.

use super::schema::{MatchResult, ProducersEntry, Reversed, SectionDiff};
use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub struct Section {
    pub id: u8,
    pub name: Option<String>,
    pub body: Vec<u8>,
}

fn read_uleb(buf: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift = 0;
    loop {
        let byte = *buf.get(*pos)?;
        *pos += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
}

fn read_str(buf: &[u8], pos: &mut usize) -> Option<String> {
    let len = read_uleb(buf, pos)? as usize;
    let end = pos.checked_add(len)?;
    let bytes = buf.get(*pos..end)?;
    *pos = end;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

pub fn section_kind(id: u8) -> &'static str {
    match id {
        0 => "custom",
        1 => "type",
        2 => "import",
        3 => "function",
        4 => "table",
        5 => "memory",
        6 => "global",
        7 => "export",
        8 => "start",
        9 => "element",
        10 => "code",
        11 => "data",
        12 => "datacount",
        13 => "tag",
        _ => "unknown",
    }
}

pub fn is_wasm(w: &[u8]) -> bool {
    w.len() >= 8 && &w[0..4] == b"\0asm"
}

pub fn walk(wasm: &[u8]) -> Vec<Section> {
    let mut out = Vec::new();
    if !is_wasm(wasm) {
        return out;
    }
    let mut pos = 8usize;
    while pos < wasm.len() {
        let id = wasm[pos];
        pos += 1;
        let Some(size) = read_uleb(wasm, &mut pos) else { break };
        let size = size as usize;
        let end = match pos.checked_add(size) {
            Some(e) if e <= wasm.len() => e,
            _ => break,
        };
        let body = wasm[pos..end].to_vec();
        let name = if id == 0 {
            let mut cp = 0usize;
            read_str(&body, &mut cp)
        } else {
            None
        };
        out.push(Section { id, name, body });
        pos = end;
    }
    out
}

fn parse_producers(body: &[u8]) -> Vec<ProducersEntry> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let Some(fields) = read_uleb(body, &mut pos) else { return out };
    for _ in 0..fields {
        let Some(fname) = read_str(body, &mut pos) else { break };
        let Some(vcount) = read_uleb(body, &mut pos) else { break };
        for _ in 0..vcount {
            let Some(name) = read_str(body, &mut pos) else { break };
            let Some(version) = read_str(body, &mut pos) else { break };
            out.push(ProducersEntry { field: fname.clone(), name, version });
        }
    }
    out
}

const PATH_MARKERS: &[&str] = &[
    "/home/", "/Users/", "/root/", "/build/", "/rustc/", ".cargo/registry/src/",
    "/usr/local/cargo/", "/.rustup/",
];

fn is_path_char(b: u8) -> bool {
    b.is_ascii_graphic() || b == b' '
}

fn mine_paths(wasm: &[u8]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < wasm.len() {
        if !is_path_char(wasm[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < wasm.len() && is_path_char(wasm[i]) {
            i += 1;
        }
        let Ok(s) = std::str::from_utf8(&wasm[start..i]) else { continue };
        if s.len() < 6 {
            continue;
        }
        let is_delim = |c: char| c.is_whitespace() || "\"'`(),;:<>[]{}|\\".contains(c);
        for marker in PATH_MARKERS {
            if let Some(mpos) = s.find(marker) {
                let lead = s[..mpos].rfind(is_delim).map(|p| p + 1).unwrap_or(0);
                let mut path = s[lead..].to_string();
                if let Some(end) = path.find(is_delim) {
                    path.truncate(end);
                }
                let path = path.trim().to_string();
                if path.len() >= 6 && path.starts_with('/') && !found.contains(&path) {
                    found.push(path);
                }
            }
        }
    }
    found.sort();
    found.truncate(96);
    found
}

/// Reverse the build environment from a target wasm.
pub fn reverse(wasm: &[u8]) -> Reversed {
    let mut r = Reversed {
        sha256: sha256_hex(wasm),
        size: wasm.len(),
        ..Default::default()
    };
    if !is_wasm(wasm) {
        r.notes.push("not a wasm module (bad magic)".into());
        return r;
    }
    r.is_wasm = true;
    for s in walk(wasm) {
        if s.id == 0 {
            if let Some(name) = &s.name {
                r.custom_sections.push(name.clone());
                if name == "name" {
                    r.has_name_section = true;
                } else if name == "producers" {
                    r.has_producers_section = true;
                    // body includes [namelen][name]; re-skip it
                    let mut cp = 0usize;
                    let _ = read_str(&s.body, &mut cp);
                    r.producers = parse_producers(&s.body[cp..]);
                }
            }
        }
    }
    for p in &r.producers {
        if p.field != "processed-by" {
            continue;
        }
        let l = p.name.to_lowercase();
        if p.name == "rustc" {
            r.rustc = Some(p.version.clone());
        } else if l.contains("clang") || l.contains("llvm") {
            r.clang_vendor = Some(p.name.clone());
            r.clang_version = Some(p.version.clone());
        }
    }
    r.embedded_paths = mine_paths(wasm);
    let mut real_home = false;
    for path in &r.embedded_paths {
        // HOME
        for pre in ["/home/", "/Users/"] {
            if path.starts_with(pre) {
                real_home = true;
                if r.home.is_none() {
                    // HOME = /home/<user> or /Users/<user>
                    let rest = &path[pre.len()..];
                    if let Some(slash) = rest.find('/') {
                        r.home = Some(format!("{}{}", pre, &rest[..slash]));
                    }
                }
            }
        }
        if let Some(idx) = path.find(".cargo/registry/src/") {
            let tail = &path[idx + ".cargo/registry/src/".len()..];
            let seg: Vec<&str> = tail.splitn(3, '/').collect();
            if !seg.is_empty() && seg[0].starts_with("index.crates.io-") && r.registry_hash.is_none() {
                r.registry_hash = Some(seg[0].to_string());
            }
            if seg.len() >= 2 {
                let cv = seg[1].to_string();
                if cv.contains('-') && !r.dep_versions.contains(&cv) {
                    r.dep_versions.push(cv);
                }
            }
        }
        if let Some(g) = path.find(".cargo/git/checkouts/") {
            let tail = &path[g + ".cargo/git/checkouts/".len()..];
            let seg: Vec<&str> = tail.splitn(3, '/').collect();
            if seg.len() >= 2 {
                let name = seg[0].rfind('-').map(|p| &seg[0][..p]).unwrap_or(seg[0]).to_string();
                let rev = seg[1].to_string();
                if !r.git_checkouts.iter().any(|(n, rv)| n == &name && rv == &rev) {
                    r.git_checkouts.push((name, rev));
                }
            }
        }
        // rustc toolchain triple: .../.rustup/toolchains/<triple>/lib/rustlib/...
        if let Some(t) = path.find("/toolchains/") {
            let tail = &path[t + "/toolchains/".len()..];
            if let Some(slash) = tail.find('/') {
                let triple = &tail[..slash];
                if triple.contains('-') && r.host_triple.is_none() {
                    r.host_triple = Some(triple.to_string());
                }
            }
        }
        // rustc commit: /rustc/<hash>/...
        if path.starts_with("/rustc/") && r.rustc_commit.is_none() {
            let rest = &path["/rustc/".len()..];
            if let Some(slash) = rest.find('/') {
                r.rustc_commit = Some(rest[..slash].to_string());
            }
        }
        // local build dir (source path not in registry/rustc)
        if r.build_dir.is_none()
            && path.contains("/src/")
            && !path.contains(".cargo/registry")
            && !path.starts_with("/rustc/")
            && !path.contains("/.rustup/")
        {
            if let Some(sidx) = path.find("/src/") {
                r.build_dir = Some(path[..sidx].to_string());
            }
        }
    }
    let remap_style = r
        .embedded_paths
        .iter()
        .any(|p| p.starts_with("/rustc/") || p.starts_with("/build") || p.starts_with("/cargo/"));
    r.remapped = remap_style && !real_home;
    r.links_c = r.dep_versions.iter().any(|d| d.starts_with("secp256k1-sys"))
        || String::from_utf8_lossy(wasm).contains("rustsecp256k1");
    r.dep_versions.sort();
    r.dep_versions.dedup();
    r
}

/// Diff a rebuilt candidate against the target wasm. Custom sections (producers/name)
/// are EXCLUDED from the normalized score (build metadata), but a full sha256 match is
/// what earns `reproducible`.
pub fn compare(target: &[u8], candidate: &[u8]) -> MatchResult {
    use std::collections::{BTreeMap, BTreeSet};
    let key = |s: &Section| match &s.name {
        Some(n) => format!("custom:{n}"),
        None => section_kind(s.id).to_string(),
    };
    let tw = walk(target);
    let cw = walk(candidate);
    let mut tm: BTreeMap<String, &Section> = BTreeMap::new();
    let mut cm: BTreeMap<String, &Section> = BTreeMap::new();
    for s in &tw {
        tm.insert(key(s), s);
    }
    for s in &cw {
        cm.insert(key(s), s);
    }
    let mut sections = Vec::new();
    let mut code_match = false;
    let (mut norm_total, mut norm_matched) = (0usize, 0usize);
    let all: BTreeSet<String> = tm.keys().chain(cm.keys()).cloned().collect();
    for k in all {
        let tb = tm.get(&k).map(|s| s.body.as_slice()).unwrap_or(&[]);
        let cb = cm.get(&k).map(|s| s.body.as_slice()).unwrap_or(&[]);
        let matched = tb == cb;
        if k == "code" {
            code_match = matched;
        }
        if !k.starts_with("custom:") {
            let w = tb.len().max(cb.len());
            norm_total += w;
            if matched {
                norm_matched += w;
            }
        }
        sections.push(SectionDiff {
            kind: k,
            target_size: tb.len(),
            candidate_size: cb.len(),
            matched,
        });
    }
    let normalized_match_pct = if norm_total > 0 {
        100.0 * norm_matched as f64 / norm_total as f64
    } else {
        0.0
    };
    let byte_exact = target == candidate;
    let verdict = if byte_exact {
        "reproducible"
    } else if code_match && normalized_match_pct >= 99.5 {
        "verified"
    } else if code_match {
        "code_match"
    } else if normalized_match_pct >= 98.0 {
        "partial"
    } else {
        "mismatch"
    }
    .to_string();
    MatchResult {
        verdict,
        built_sha256: sha256_hex(candidate),
        byte_exact,
        normalized_match_pct,
        sections,
        note: None,
    }
}
