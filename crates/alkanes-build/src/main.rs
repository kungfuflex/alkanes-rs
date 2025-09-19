use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::process::{Command, Stdio};

fn compress(binary: Vec<u8>) -> Result<Vec<u8>> {
    let mut writer = GzEncoder::new(Vec::<u8>::with_capacity(binary.len()), Compression::best());
    writer.write_all(&binary)?;
    Ok(writer.finish()?)
}

fn build_alkane(wasm_str: &str, features: Vec<&str>) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.env("CARGO_TARGET_DIR", wasm_str)
        .arg("build")
        .arg("--target=wasm32-unknown-unknown")
        .arg("--release");

    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }

    let status = cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn()?.wait()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Failed to build alkane with features: {:?}", features))
    }
}

fn main() -> Result<()> {
    let project_root = env::current_dir()?;
    let target_dir = project_root.join("target");
    let wasm_dir = target_dir.join("alkanes-wasm");
    fs::create_dir_all(&wasm_dir)?;
    let wasm_str = wasm_dir.to_str().unwrap();
    let write_dir = project_root.join("crates").join("alkanes").join("src").join("precompiled");
    let crates_dir = project_root.join("crates");

    fs::create_dir_all(&write_dir.join("std"))?;
    
    let mods = fs::read_dir(&crates_dir)?
        .filter_map(|v| {
            let name = v.ok()?.file_name().into_string().ok()?;
            if name.starts_with("alkanes-std-") {
                Some(name)
            } else {
                None
            }
        })
        .collect::<Vec<String>>();

    for v in &mods {
        let current_crate_dir = crates_dir.join(v);
        if !current_crate_dir.join("Cargo.toml").exists() {
            continue;
        }
        std::env::set_current_dir(&current_crate_dir)?;
        
        let subbed = v.replace('-', "_");

        if v == "alkanes-std-genesis-alkane"
            || v == "alkanes-std-genesis-alkane-upgraded"
            || v == "alkanes-std-merkle-distributor"
        {
            let precompiled_dir = write_dir.join("precompiled");
            fs::create_dir_all(&precompiled_dir)?;

            let networks = vec![
                ("bellscoin", vec!["bellscoin"]),
                ("luckycoin", vec!["luckycoin"]),
                ("mainnet", vec!["mainnet"]),
                ("fractal", vec!["fractal"]),
                ("regtest", vec!["regtest"]),
                ("testnet", vec!["regtest"]),
            ];

            for (network, features) in networks {
                build_alkane(wasm_str, features)?;
                let wasm_file_name = subbed.clone();
                let file_path = Path::new(wasm_str)
                    .join("wasm32-unknown-unknown")
                    .join("release")
                    .join(format!("{}.wasm", wasm_file_name));

                let f: Vec<u8> = fs::read(&file_path)?;
                let final_wasm_path = wasm_dir.join(format!("{}_{}.wasm", subbed, network));
                fs::write(&final_wasm_path, &f)?;

                let generated_file_content = format!(
                    "pub fn get_bytes() -> Vec<u8> {{ include_bytes!(\"{}\").to_vec() }}",
                    final_wasm_path.to_str().unwrap().replace('\\', "/")
                );
                fs::write(
                    &write_dir.join("std").join(format!("{}_{}_build.rs", subbed, network)),
                    generated_file_content,
                )?;
            }
            build_alkane(wasm_str, vec!["regtest"])?;
        } else {
            build_alkane(wasm_str, vec![])?;
        }

        let wasm_file_name = subbed.clone();
        let file_path = Path::new(wasm_str)
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(format!("{}.wasm", wasm_file_name));
        
        if file_path.exists() {
            let f: Vec<u8> = fs::read(&file_path)?;
            let compressed: Vec<u8> = compress(f.clone())?;
            fs::write(
                &wasm_dir.join(format!("{}.wasm.gz", subbed)),
                &compressed,
            )?;

            let generated_file_content = format!(
                "pub fn get_bytes() -> Vec<u8> {{ include_bytes!(\"{}\").to_vec() }}",
                file_path.to_str().unwrap().replace('\\', "/")
            );
            fs::write(
                &write_dir.join("std").join(format!("{}_build.rs", subbed)),
                generated_file_content,
            )?;
        }
    }
    
    std::env::set_current_dir(&project_root)?;

    let mut mod_content = mods
        .iter()
        .map(|v| v.replace('-', "_"))
        .fold(String::new(), |r, v| {
            r + "pub mod " + &v + "_build;\n"
        });

    let networks = [
        "bellscoin",
        "luckycoin",
        "mainnet",
        "fractal",
        "regtest",
        "testnet",
    ];
    let genesis_base = "alkanes_std_genesis_alkane";
    for network in networks {
        mod_content.push_str(&format!("pub mod {}_{}_build;\n", genesis_base, network));
    }

    let merkle_base = "alkanes_std_merkle_distributor";
    for network in networks {
        mod_content.push_str(&format!("pub mod {}_{}_build;\n", merkle_base, network));
    }
    fs::write(&write_dir.join("std").join("mod.rs"), mod_content)?;

    Ok(())
}