//! Generate the reproduction build script (the "formula") from a `BuildInfo`. This is
//! the generalization of the hand-written repro-*.sh scripts: given the fully
//! parameterized BuildInfo, emit a self-contained bash script that reconstructs the
//! EXACT build environment and produces the wasm, run inside a clean container.
//!
//! The formula composes the techniques we proved: darwin toolchain-path copy, HOME
//! reconstruction, matched clang (apt / LLVM.org / assembled-Homebrew-from-GHCR),
//! registry time-machine served AS index.crates.io, and bare git source-ids.

use super::schema::BuildInfo;

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Emit the full bash script. It expects to run as root in the `toolchain.os_image`
/// container with `/out` mounted for the result and `--add-host index.crates.io:127.0.0.1`.
pub fn generate_build_script(bi: &BuildInfo) -> String {
    let mut s = String::new();
    s.push_str("#!/usr/bin/env bash\nset -uo pipefail\nexport DEBIAN_FRONTEND=noninteractive\n");
    s.push_str("apt-get update -qq && apt-get install -y -qq curl git python3 ca-certificates openssl xz-utils patchelf pkg-config protobuf-compiler build-essential libffi8 libncurses6 zlib1g libzstd1 >/dev/null 2>&1\n\n");

    // ── environment: HOME + toolchain-at-darwin-path ──────────────────────────
    let home = &bi.environment.home;
    let cargo_home = bi.environment.cargo_home.clone().unwrap_or_else(|| format!("{home}/.cargo"));
    s.push_str(&format!("export HOME={}\n", sh_quote(home)));
    s.push_str(&format!("export RUSTUP_HOME={}/.rustup CARGO_HOME={}\n", sh_quote(home), sh_quote(&cargo_home)));
    s.push_str(&format!("mkdir -p {}\n", sh_quote(home)));
    s.push_str("curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal >/dev/null 2>&1\n");
    s.push_str(&format!("export PATH={}/bin:$PATH\n", sh_quote(&cargo_home)));
    let rustc = &bi.toolchain.rustc_version;
    s.push_str(&format!(
        "rustup toolchain install {} --profile minimal --target {} --component rust-src --no-self-update >/dev/null 2>&1\n",
        sh_quote(rustc), sh_quote(&bi.toolchain.target)
    ));
    // physically COPY the linux toolchain to the reversed host_triple path (symlink fails)
    s.push_str(&format!(
        "LINUXTC=$RUSTUP_HOME/toolchains/{}-x86_64-unknown-linux-gnu\n",
        sh_quote(rustc).trim_matches('\'')
    ));
    s.push_str(&format!(
        "[ -d \"$RUSTUP_HOME/toolchains/{tc}\" ] || cp -a \"$LINUXTC\" \"$RUSTUP_HOME/toolchains/{tc}\"\n",
        tc = bi.toolchain.host_triple
    ));
    s.push_str(&format!("TC={}\n", sh_quote(&bi.toolchain.host_triple)));
    s.push_str("echo \"rustc: $(rustc +$TC --version 2>&1)\"\n\n");

    // ── C toolchain (clang) ───────────────────────────────────────────────────
    if let Some(ct) = &bi.c_toolchain {
        match ct.source.method.as_str() {
            "apt" => {
                if let Some(pkg) = &ct.source.apt_package {
                    s.push_str(&format!("apt-get install -y -qq {} >/dev/null 2>&1\n", pkg));
                    s.push_str(&format!("export CC_wasm32_unknown_unknown={}\n", pkg));
                }
            }
            "llvm-release" => {
                s.push_str(&format!(
                    "curl -sSL https://github.com/llvm/llvm-project/releases/download/llvmorg-{v}/LLVM-{v}-Linux-X64.tar.xz -o /tmp/llvm.tar.xz\n",
                    v = ct.version
                ));
                s.push_str("mkdir -p /opt/llvm && tar -xf /tmp/llvm.tar.xz -C /opt/llvm --strip-components=1 2>/dev/null\n");
                s.push_str("export CC_wasm32_unknown_unknown=/opt/llvm/bin/clang AR_wasm32_unknown_unknown=/opt/llvm/bin/llvm-ar\n");
            }
            "homebrew-bottle" => {
                // assemble Homebrew clang on Linux from GHCR bottle blobs + patchelf
                s.push_str("HBP=/opt/hb; mkdir -p $HBP\n");
                s.push_str("tok(){ curl -s \"https://ghcr.io/token?scope=repository:homebrew/core/$1:pull\" | python3 -c \"import sys,json;print(json.load(sys.stdin)['token'])\"; }\n");
                s.push_str("pull(){ curl -s -L -H \"Authorization: Bearer $(tok \"$1\")\" \"https://ghcr.io/v2/homebrew/core/$1/blobs/sha256:$2\" -o \"$3\"; }\n");
                for b in &ct.source.homebrew_bottles {
                    s.push_str(&format!("pull {} {} /tmp/{}.tgz\n", b.formula, b.blob_sha256, b.formula));
                    s.push_str(&format!("tar -xzf /tmp/{}.tgz -C $HBP 2>/dev/null\n", b.formula));
                }
                s.push_str("LLVMDIR=$(ls -d $HBP/llvm/*/ | head -1); LLVMDIR=${LLVMDIR%/}\n");
                s.push_str("cp -a $HBP/z3/*/lib/libz3.so* \"$LLVMDIR/lib/\" 2>/dev/null || true\n");
                s.push_str("cp -a $HBP/libedit/*/lib/libedit.so* \"$LLVMDIR/lib/\" 2>/dev/null || true\n");
                s.push_str("SYS=/lib/x86_64-linux-gnu\n");
                s.push_str("for b in \"$LLVMDIR/lib/libLLVM.so.\"* \"$LLVMDIR/lib/libclang-cpp.so.\"*; do [ -f \"$b\" ] && patchelf --set-rpath \"$LLVMDIR/lib:$SYS\" \"$b\"; done\n");
                s.push_str("for t in clang-* llvm-ar llvm-ranlib llvm-nm; do f=\"$LLVMDIR/bin/$t\"; [ -f \"$f\" ] || continue; real=$(readlink -f \"$f\"); patchelf --set-rpath \"$LLVMDIR/lib:$SYS\" \"$real\" 2>/dev/null; patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 \"$real\" 2>/dev/null; done\n");
                s.push_str("CLANGBIN=$(ls $LLVMDIR/bin/clang-* | grep -E 'clang-[0-9]+$' | head -1)\n");
                s.push_str("export PATH=\"$LLVMDIR/bin:$PATH\"\n");
                s.push_str("export CC_wasm32_unknown_unknown=\"$CLANGBIN\" AR_wasm32_unknown_unknown=\"$LLVMDIR/bin/llvm-ar\"\n");
            }
            _ => {}
        }
        s.push_str("echo \"clang: $(${CC_wasm32_unknown_unknown:-clang} --version 2>&1 | head -1)\"\n\n");
    }

    // extra build-env exports
    for kv in &bi.environment.build_env {
        s.push_str(&format!("export {}\n", kv));
    }
    if !bi.environment.remap_path_prefix.is_empty() {
        let flags: Vec<String> = bi
            .environment
            .remap_path_prefix
            .iter()
            .map(|r| format!("--remap-path-prefix={r}"))
            .collect();
        s.push_str(&format!("export RUSTFLAGS={}\n", sh_quote(&flags.join(" "))));
    }

    // ── registry ──────────────────────────────────────────────────────────────
    if bi.registry.mode == "time-machine" {
        if let Some(commit) = &bi.registry.archive_commit {
            s.push_str(&registry_time_machine(commit));
        }
    } // committed-lock ⇒ real crates.io + --locked (nothing to set up)

    // ── source ────────────────────────────────────────────────────────────────
    let src_root = if !bi.source.inline_files.is_empty() {
        // self-contained: write the inline files under the build_dir/home
        let root = bi.source.subdir.clone().unwrap_or_else(|| format!("{home}/src"));
        s.push_str(&format!("mkdir -p {}\n", sh_quote(&root)));
        for f in &bi.source.inline_files {
            let full = format!("{root}/{}", f.path);
            let dir = std::path::Path::new(&full).parent().map(|p| p.display().to_string()).unwrap_or_default();
            s.push_str(&format!("mkdir -p {}\n", sh_quote(&dir)));
            if let Some(b64) = f.content.strip_prefix("base64:") {
                s.push_str(&format!("echo {} | base64 -d > {}\n", sh_quote(b64), sh_quote(&full)));
            } else {
                s.push_str(&format!("cat > {} <<'ALKEOF'\n{}\nALKEOF\n", sh_quote(&full), f.content));
            }
        }
        root
    } else {
        // git ref — clone at the exact build_dir path (embedded paths must match)
        let repo = bi.source.repo.clone().unwrap_or_default();
        let dst = bi
            .source
            .subdir
            .clone()
            .unwrap_or_else(|| format!("{home}/src"));
        let parent = std::path::Path::new(&dst).parent().map(|p| p.display().to_string()).unwrap_or_else(|| home.clone());
        s.push_str(&format!("mkdir -p {} && cd {}\n", sh_quote(&parent), sh_quote(&parent)));
        s.push_str(&format!("rm -rf {dst} && git clone -q {repo} {dst} 2>/dev/null\n", dst = sh_quote(&dst), repo = sh_quote(&repo)));
        if let Some(commit) = &bi.source.commit {
            s.push_str(&format!("git -C {} checkout -q {}\n", sh_quote(&dst), sh_quote(commit)));
        }
        dst
    };
    s.push_str(&format!("cd {}\n", sh_quote(&src_root)));
    s.push_str("rm -f rust-toolchain.toml rust-toolchain 2>/dev/null || true\n");
    s.push_str("mkdir -p .cargo && printf '\\n[net]\\ngit-fetch-with-cli = true\\n' >> .cargo/config.toml\n");
    if bi.registry.mode == "time-machine" {
        s.push_str("grep -q '^rust-version' Cargo.toml 2>/dev/null || sed -i '/^edition = /a rust-version = \"1.82.0\"' Cargo.toml 2>/dev/null || true\n");
        s.push_str("printf '\\n[resolver]\\nincompatible-rust-versions = \"fallback\"\\n' >> .cargo/config.toml\n");
    }

    // ── git-dep source-id handling (bare) ─────────────────────────────────────
    let locked_flag = if bi.git_deps.iter().any(|g| g.source_form == "bare") { "--locked" } else { "" };
    for g in &bi.git_deps {
        if g.source_form == "bare" {
            // resolve WITH the rev, then bare the lockfile source-id + build --locked
            let esc_url = g.url.replace('/', "\\/").replace('&', "\\&");
            s.push_str(&format!(
                "sed -i 's@git = \"{u}\"@git = \"{u}\", rev = \"{r}\"@g' Cargo.toml\n",
                u = g.url, r = g.rev
            ));
            let _ = esc_url;
        }
    }
    s.push_str("cargo +$TC generate-lockfile 2>&1 | tail -2\n");
    for g in &bi.git_deps {
        if g.source_form == "bare" {
            s.push_str(&format!("sed -i 's@, rev = \"{r}\"@@g' Cargo.toml\n", r = g.rev));
            s.push_str(&format!("sed -i 's@?rev={r}#@#@g' Cargo.lock\n", r = g.rev));
        }
    }

    // ── build ─────────────────────────────────────────────────────────────────
    let mut build = format!("cargo +$TC build --release {locked_flag} --target {}", bi.toolchain.target);
    if !bi.source.features.is_empty() {
        build.push_str(&format!(" --features {}", bi.source.features.join(",")));
    }
    if let Some(pkg) = &bi.source.package {
        build.push_str(&format!(" -p {pkg}"));
    }
    s.push_str(&format!("{build} >/tmp/build.log 2>&1 && echo 'BUILD OK' || {{ echo 'BUILD FAIL'; tail -20 /tmp/build.log; }}\n"));
    // locate + emit the wasm
    let art = bi.source.artifact.clone().unwrap_or_default();
    s.push_str(&format!(
        "W=$(ls -S target/{t}/release/{a}*.wasm 2>/dev/null | head -1)\n",
        t = bi.toolchain.target,
        a = if art.is_empty() { "".into() } else { art.trim_end_matches(".wasm").to_string() }
    ));
    s.push_str(&format!("[ -z \"$W\" ] && W=$(ls -S {home}/*/target/{t}/release/*.wasm target/{t}/release/*.wasm 2>/dev/null | head -1)\n", home = home, t = bi.toolchain.target));
    s.push_str("if [ -n \"$W\" ]; then echo \"built: $W sha=$(sha256sum \"$W\"|cut -c1-16) size=$(stat -c%s \"$W\")\"; cp \"$W\" /out/built.wasm; else echo 'NO WASM'; fi\n");
    s
}

fn registry_time_machine(archive_commit: &str) -> String {
    format!(
        r#"# registry time-machine: crates.io-index tree frozen at the build date, served AS index.crates.io
mkdir -p /idx && cd /idx && git init -q
git remote add origin https://github.com/rust-lang/crates.io-index-archive
git fetch -q --depth 1 origin {commit} && git checkout -q FETCH_HEAD || {{ echo FREEZE-FAIL; exit 1; }}
echo '{{"dl":"https://static.crates.io/crates/{{crate}}/{{crate}}-{{version}}.crate","api":"https://crates.io"}}' > config.json
openssl req -x509 -newkey rsa:2048 -keyout /idx/key.pem -out /idx/cert.pem -days 3650 -nodes -subj "/CN=index.crates.io" -addext "subjectAltName=DNS:index.crates.io" >/dev/null 2>&1
cat /idx/cert.pem /etc/ssl/certs/ca-certificates.crt > /idx/cabundle.pem
grep -q index.crates.io /etc/hosts || echo "127.0.0.1 index.crates.io" >> /etc/hosts
cat > /idx/serve.py <<'PYEOF'
import http.server, ssl, os
os.chdir('/idx'); ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); ctx.load_cert_chain('/idx/cert.pem','/idx/key.pem')
h=http.server.ThreadingHTTPServer(('127.0.0.1',443), http.server.SimpleHTTPRequestHandler)
h.socket=ctx.wrap_socket(h.socket, server_side=True); h.serve_forever()
PYEOF
nohup python3 /idx/serve.py >/tmp/idx.log 2>&1 & sleep 2
export CARGO_HTTP_CAINFO=/idx/cabundle.pem
echo "frozen index: $(curl -s --cacert /idx/cabundle.pem -o /dev/null -w '%{{http_code}}' https://index.crates.io/config.json)"
"#,
        commit = archive_commit
    )
}
