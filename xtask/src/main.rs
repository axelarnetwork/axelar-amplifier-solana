use std::str::FromStr;
use std::{collections::HashMap, path::PathBuf};

use clap::{Parser, Subcommand};
use eyre::OptionExt;
use itertools::Itertools;
use xshell::{cmd, Shell};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Test {
        /// Will test contracts by default using sbf-test.
        /// This flag will ensure that we also run non-sbf tests
        #[clap(short, long, default_value_t = false)]
        only_sbf: bool,
    },
    Build {
        /// Network environment: devnet-amplifier, stagenet, testnet, or mainnet
        #[clap(short, long)]
        network: Option<String>,
    },
    Check,
    Fmt,
    UnusedDeps,
    Typos,
    Docs,
    Audit {
        #[clap(last = true)]
        args: Vec<String>,
    },
    Deny {
        #[clap(last = true)]
        args: Vec<String>,
    },
    UpdateIds {
        /// Network environment: devnet-amplifier, stagenet, testnet, or mainnet
        #[clap(short, long)]
        network: String,
    },
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let sh = Shell::new()?;
    let args = Args::parse();

    match args.command {
        Commands::Test { only_sbf } => {
            println!("cargo test");
            let (solana_programs, auxiliary_crates) = workspace_crates_by_category(&sh)?;

            // build all solana programs (because they have internal inter-dependencies)
            for (_program, path) in solana_programs.iter() {
                let manifest_path = path.join("Cargo.toml");
                cmd!(sh, "cargo build-sbf --manifest-path {manifest_path}").run()?;
            }

            // test solana programs using `test-sbf`
            for (program, ..) in solana_programs.iter() {
                cmd!(sh, "cargo test-sbf -p {program}").run()?;
            }
            if only_sbf {
                return Ok(());
            }

            let auxiliary_args = auxiliary_crates
                .iter()
                .flat_map(|(crate_name, _)| ["-p", crate_name])
                .collect::<Vec<_>>();

            if !auxiliary_args.is_empty() {
                cmd!(sh, "cargo test {auxiliary_args...}").run()?;
            }
        }
        Commands::Build { network } => {
            println!("cargo build");

            // Validate network parameter if provided
            let valid_networks = ["devnet-amplifier", "stagenet", "testnet", "mainnet"];
            if let Some(ref net) = network {
                if !valid_networks.contains(&net.as_str()) {
                    return Err(eyre::eyre!(
                        "Invalid network '{}'. Must be one of: devnet-amplifier, stagenet, testnet, mainnet",
                        net
                    ));
                }
            }

            let (solana_programs, _auxiliary_crates) = workspace_crates_by_category(&sh)?;

            // build all solana programs (because they have internal inter-dependencies)
            for (_program, path) in solana_programs.iter() {
                let manifest_path = path.join("Cargo.toml");

                if let Some(ref net) = network {
                    println!("Building with network feature: {net}");
                    cmd!(
                        sh,
                        "cargo build-sbf --manifest-path {manifest_path} --features {net} --no-default-features"
                    )
                    .run()?;
                } else {
                    cmd!(sh, "cargo build-sbf --manifest-path {manifest_path}").run()?;
                }
            }
        }
        Commands::Check => {
            println!("cargo check");
            cmd!(
                sh,
                "cargo clippy --no-deps --all-targets --workspace --locked -- -D warnings"
            )
            .run()?;
            // For some reason cargo clippy doesn't catch all issues in solana-axelar-std
            // TODO check why and possibly remove this extra check
            cmd!(sh, "cargo check -p solana-axelar-std").run()?;
            cmd!(sh, "cargo fmt --all --check").run()?;
        }
        Commands::Fmt => {
            println!("cargo fix");
            cmd!(sh, "cargo fmt --all").run()?;
            cmd!(
                sh,
                "cargo fix --allow-dirty --allow-staged --workspace --tests"
            )
            .run()?;
            cmd!(
                sh,
                "cargo clippy --fix --allow-dirty --allow-staged --workspace --tests"
            )
            .run()?;
        }
        Commands::UnusedDeps => {
            println!("unused deps");
            cmd!(sh, "cargo +nightly install cargo-machete").run()?;
            cmd!(sh, "cargo-machete").run()?;
        }
        Commands::Typos => {
            println!("typos check");
            cmd!(sh, "cargo install typos-cli").run()?;
            cmd!(sh, "typos").run()?;
        }
        Commands::Docs => {
            println!("cargo doc");
            cmd!(sh, "cargo doc --workspace --no-deps").run()?;

            if std::option_env!("CI").is_none() {
                #[cfg(target_os = "macos")]
                cmd!(sh, "open target/doc/relayer/index.html").run()?;

                #[cfg(target_os = "linux")]
                cmd!(sh, "xdg-open target/doc/relayer/index.html").run()?;
            }
        }
        Commands::Audit { args } => {
            println!("cargo audit");
            cmd!(
                sh,
                "cargo install cargo-audit --version 0.22.0 --force --locked"
            )
            .run()?;
            cmd!(sh, "cargo audit --version").run()?;
            cmd!(sh, "cargo audit {args...}").run()?;
        }
        Commands::Deny { args } => {
            println!("cargo deny");
            cmd!(sh, "cargo +nightly install cargo-deny").run()?;
            cmd!(sh, "cargo deny check {args...}").run()?;
        }
        Commands::UpdateIds { network } => {
            let valid_networks = ["devnet-amplifier", "stagenet", "testnet", "mainnet"];
            if !valid_networks.contains(&network.as_str()) {
                return Err(eyre::eyre!(
                    "Invalid network '{}'. Must be one of: devnet-amplifier, stagenet, testnet, mainnet",
                    network
                ));
            }

            println!("Updating program IDs for network: {network}");

            let program_prefixes = [
                ("solana-axelar-gas-service", "gas"),
                ("solana-axelar-gateway", "gtw"),
                ("solana-axelar-governance", "gov"),
                ("solana-axelar-its", "its"),
                ("solana-axelar-memo", "mem"),
                ("solana-axelar-operators", "opr"),
            ];

            let (solana_programs, _) = workspace_crates_by_category(&sh)?;

            let solana_network = match network.as_str() {
                "devnet-amplifier" => "devnet",
                other => other,
            };
            let mut anchor_toml_block = format!("[programs.{solana_network}]\n");

            for (program_name, program_path) in solana_programs {
                if let Some((_, prefix)) = program_prefixes
                    .iter()
                    .find(|(name, _)| program_name == *name)
                {
                    println!("Regenerating ID for {program_name} with prefix {prefix}");
                    let lib_rs_path = program_path.join("src/lib.rs");

                    if !lib_rs_path.exists() {
                        println!("Warning: {lib_rs_path:?} not found, skipping");
                        continue;
                    }

                    // Generate new program ID using solana-keygen grind
                    let output =
                        cmd!(sh, "solana-keygen grind --starts-with {prefix}:1").output()?;

                    // Parse the output to extract the pubkey
                    let output_str = String::from_utf8(output.stdout)?;
                    let mut new_id = String::new();
                    for line in output_str.lines() {
                        if line.contains(".json") {
                            if let Some(filename) = line.split_whitespace().last() {
                                if let Some(pubkey) = filename.split('.').next() {
                                    new_id = pubkey.to_string();
                                    break;
                                }
                            }
                        }
                    }

                    if new_id.is_empty() {
                        println!("Failed to generate new ID for {program_name}");
                        continue;
                    }

                    println!("Generated new ID for {program_name}: {new_id}");

                    let program_name_snake = program_name.replace("-", "_");
                    anchor_toml_block.push_str(&format!("{program_name_snake} = \"{new_id}\"\n"));

                    // Update the declare_id! for the target network in lib.rs
                    let lib_content = std::fs::read_to_string(&lib_rs_path)?;
                    let lines: Vec<&str> = lib_content.lines().collect();
                    let cfg_marker = format!("#[cfg(feature = \"{network}\")]");

                    let cfg_idx = lines
                        .iter()
                        .position(|line| line.trim() == cfg_marker)
                        .ok_or_eyre(format!("Could not find {cfg_marker} in {lib_rs_path:?}"))?;

                    // The declare_id! is on the next line
                    let declare_idx = cfg_idx + 1;
                    eyre::ensure!(
                        declare_idx < lines.len()
                            && lines[declare_idx].trim().starts_with("declare_id!("),
                        "Expected declare_id! on line after {cfg_marker} in {lib_rs_path:?}"
                    );

                    let old_declare_line = lines[declare_idx];
                    let new_declare_line = format!("declare_id!(\"{new_id}\");");
                    let updated_content =
                        lib_content.replacen(old_declare_line, &new_declare_line, 1);

                    std::fs::write(&lib_rs_path, updated_content)?;
                    println!("Updated declare_id! for {network} in {lib_rs_path:?}");
                }
            }

            println!("Program IDs regenerated and successfully updated for {network}");
            println!("You can update Anchor.toml for {network}:\n{anchor_toml_block}");
        }
    }

    Ok(())
}

type WorkspaceCrateInfo<'a> = (&'a str, PathBuf);

/// Return all crates in the workspace sorted by category:
/// - (solana program crates, native crates)
fn workspace_crates_by_category(
    sh: &Shell,
) -> Result<(Vec<WorkspaceCrateInfo>, Vec<WorkspaceCrateInfo>), eyre::Error> {
    let crates_in_repo = cmd!(sh, "cargo tree --workspace --depth 0")
        .output()
        .map(|o| String::from_utf8(o.stdout))??
        .leak(); // fine to leak as xtask is short lived
    let all_crate_data = crates_in_repo.split_whitespace();
    let all_crate_data = all_crate_data
        .filter(|item| !item.starts_with('[') && !item.contains("proc-macro")) // filters "[dev-dependencies]" and "(proc-macro)"
        .tuples()
        .group_by(|(_, _, path)| path.contains("programs"));
    let mut solana_programs = HashMap::new();
    let mut auxiliary_crates = HashMap::new();
    for (is_solana_program, group) in &all_crate_data {
        for (crate_name, _crate_version, crate_path) in group {
            let crate_path = crate_path
                .strip_prefix('(')
                .ok_or_eyre("expected prefix not there")?;
            let crate_path = crate_path
                .strip_suffix(')')
                .ok_or_eyre("expected suffix not there")?;
            let crate_path = PathBuf::from_str(crate_path)?;
            if is_solana_program {
                solana_programs.insert(crate_name, crate_path);
            } else {
                auxiliary_crates.insert(crate_name, crate_path);
            }
        }
    }
    Ok((
        solana_programs.into_iter().collect(),
        auxiliary_crates.into_iter().collect(),
    ))
}
