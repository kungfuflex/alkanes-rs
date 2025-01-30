use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use hex;
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

fn build_alkane(wasm_str: &str, features: Vec<&'static str>) -> Result<()> {
    if features.len() != 0 {
        let _ = Command::new("cargo")
            .env("CARGO_TARGET_DIR", wasm_str)
            .arg("build")
            .arg("--release")
            .arg("--features")
            .arg(features.join(","))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait()?;
        Ok(())
    } else {
        Command::new("cargo")
            .env("CARGO_TARGET_DIR", wasm_str)
            .arg("build")
            .arg("--release")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait()?;
        Ok(())
    }
}

fn main() {
    println!("cargo:rerun-if-changed=crates/");
    let env_var = env::var_os("OUT_DIR").unwrap();
    let base_dir = Path::new(&env_var)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let out_dir = base_dir.join("release");
    let wasm_dir = base_dir.parent().unwrap().join("alkanes");
    fs::create_dir_all(&wasm_dir).unwrap();
    let wasm_str = wasm_dir.to_str().unwrap();
    let write_dir = Path::new(&out_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("src")
        .join("tests");

    fs::create_dir_all(&write_dir.join("std")).unwrap();
    let crates_dir = out_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates");
    match std::env::set_current_dir(&crates_dir) {
        Err(_) => return,
        _ => {}
    };
    let mods = fs::read_dir(&crates_dir)
        .unwrap()
        .filter_map(|v| {
            let name = v.ok()?.file_name().into_string().ok()?;
            if name.starts_with("alkanes-std-") {
                Some(name)
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    let files = mods
        .clone()
        .into_iter()
        .filter_map(|name| {
            let mut vars = env::vars_os();
            if let Some(feature_name) = name.strip_prefix("alkanes-std-") {
                let final_name = feature_name.to_uppercase().replace("-", "_");
                if let Some(_) = env::var(format!("CARGO_FEATURE_{}", final_name.as_str())).ok() {
                    Some(name)
                } else if vars
                    .position(|(k, _v)| k.to_owned().into_string().unwrap().contains("ALL"))
                    .is_some()
                {
                    Some(name)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    files.into_iter()
        .map(|v| -> Result<String> {
            std::env::set_current_dir(&crates_dir.clone().join(v.clone()))?;
            if v == "alkanes-std-genesis-alkane" {
                let precompiled_dir = write_dir.join("precompiled");
                fs::create_dir_all(&precompiled_dir)?;

                // Build and process for each network
                let networks = vec![
                    ("bellscoin", vec!["bellscoin"]),
                    ("luckycoin", vec!["luckycoin"]),
                    ("mainnet", vec!["mainnet"]),
                    ("fractal", vec!["fractal"]),
                    ("regtest", vec!["regtest"]),
                    ("testnet", vec!["regtest"]), // testnet uses regtest features
                ];

                for (network, features) in networks {
                    // Build with specific features
                    build_alkane(wasm_str, features)?;

                    let subbed = v.clone().replace("-", "_");

                    // Read the built wasm
                    let f: Vec<u8> = fs::read(
                        &Path::new(&wasm_str)
                            .join("wasm32-unknown-unknown")
                            .join("release")
                            .join(subbed.clone() + ".wasm"),
                    )?;

                    // Compress
                    let compressed: Vec<u8> = compress(f.clone())?;
                    fs::write(
                        &Path::new(&wasm_str)
                            .join("wasm32-unknown-unknown")
                            .join("release")
                            .join(format!("{}_{}.wasm.gz", subbed, network)),
                        &compressed
                    )?;

                    // Write network-specific build file
                }

                // Also build for the default feature set
                build_alkane(wasm_str, vec!["regtest"])?;
            } else {
                build_alkane(wasm_str, vec![])?;
            }

            std::env::set_current_dir(&crates_dir)?;
            let subbed = v.clone().replace("-", "_");
            eprintln!(
                "write: {}",
                write_dir
                    .join("std")
                    .join(subbed.clone() + "_build.rs")
                    .into_os_string()
                    .to_str()
                    .unwrap()
            );
            let f: Vec<u8> = fs::read(
                &Path::new(&wasm_str)
                    .join("wasm32-unknown-unknown")
                    .join("release")
                    .join(subbed.clone() + ".wasm"),
            )?;
            let compressed: Vec<u8> = compress(f.clone())?;
            fs::write(&Path::new(&wasm_str).join("wasm32-unknown-unknown").join("release").join(subbed.clone() + ".wasm.gz"), &compressed)?;
            let data: String = hex::encode(&f);
            fs::write(
                &write_dir.join("std").join(subbed.clone() + "_build.rs"),
                String::from("use hex_lit::hex;\n#[allow(long_running_const_eval)]\npub fn get_bytes() -> Vec<u8> { (&hex!(\"")
                    + data.as_str()
                    + "\")).to_vec() }",
            )?;
            eprintln!(
                "build: {}",
                write_dir
                    .join("std")
                    .join(subbed.clone() + "_build.rs")
                    .into_os_string()
                    .to_str()
                    .unwrap()
            );
            Ok(subbed)
        })
        .collect::<Result<Vec<String>>>()
        .unwrap();
    eprintln!(
        "write test builds to: {}",
        write_dir
            .join("std")
            .join("mod.rs")
            .into_os_string()
            .to_str()
            .unwrap()
    );
    let mut mod_content = mods
        .clone()
        .into_iter()
        .map(|v| v.replace("-", "_"))
        .fold(String::default(), |r, v| {
            r + "pub mod " + v.as_str() + "_build;\n"
        });

    // Add precompiled modules for genesis-alkane
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

    fs::write(&write_dir.join("std").join("mod.rs"), mod_content).unwrap();
    fs::write(
        &write_dir.join("std").join("mod.rs"),
        mods.into_iter()
            .map(|v| v.replace("-", "_"))
            .fold(String::default(), |r, v| {
                r + "pub mod " + v.as_str() + "_build;\n"
            }),
    )
    .unwrap();
}
