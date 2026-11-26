//! Repository consistency checks for files that live outside the contract
//! source: the pinned toolchain (#240), the generated ABI (#442), the
//! generated TypeScript bindings (#441) and the release docs (#445).
//!
//! These only read files, so they don't link the contract crate and can run
//! on their own with `cargo test --locked --test repo_consistency`.

use std::fs;
use std::path::PathBuf;

fn repo_file(path: &str) -> String {
    let full = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path} must be readable: {e}"))
}

/// Names of the `pub fn` entrypoints in `lib.rs`'s `impl OurDao` block, the
/// same set `scripts/check-readme-abi.sh` checks the README against.
fn entrypoints() -> Vec<String> {
    let lib = repo_file("contracts/dao/src/lib.rs");
    let mut in_impl = false;
    let mut names = Vec::new();
    for line in lib.lines() {
        if line.starts_with("impl OurDao") {
            in_impl = true;
        } else if in_impl && line.starts_with('}') {
            break;
        } else if in_impl {
            if let Some(rest) = line.trim_start().strip_prefix("pub fn ") {
                let name = rest.split(['(', '<']).next().unwrap();
                names.push(name.to_string());
            }
        }
    }
    assert!(!names.is_empty(), "no entrypoints found in lib.rs");
    names
}

/// The value of `key = "..."` on its own line in a TOML/YAML-ish file.
fn quoted_value(text: &str, key: &str) -> String {
    text.lines()
        .map(str::trim)
        .find_map(|line| {
            let rest = line.strip_prefix(key)?.trim_start();
            let rest = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':'))?;
            Some(rest.trim().trim_matches('"').to_string())
        })
        .unwrap_or_else(|| panic!("`{key}` not found"))
}

// ==================== #240: pinned toolchain ====================

#[test]
fn rust_toolchain_pins_an_exact_version_with_ci_components_and_target() {
    let toolchain = repo_file("rust-toolchain.toml");
    let channel = quoted_value(&toolchain, "channel");
    assert!(
        channel.split('.').count() == 3 && channel.split('.').all(|p| p.parse::<u32>().is_ok()),
        "rust-toolchain.toml must pin an exact x.y.z version, got {channel:?}"
    );
    for component in ["\"rustfmt\"", "\"clippy\""] {
        assert!(
            toolchain.contains(component),
            "missing component {component}"
        );
    }

    let wasm_target = quoted_value(&repo_file(".github/workflows/ci.yml"), "WASM_TARGET");
    assert!(
        toolchain.contains(&format!("\"{wasm_target}\"")),
        "rust-toolchain.toml targets must include ci.yml's WASM_TARGET ({wasm_target})"
    );
}

#[test]
fn docker_and_ci_use_the_pinned_toolchain() {
    let channel = quoted_value(&repo_file("rust-toolchain.toml"), "channel");

    let dockerfile = repo_file("Dockerfile.reproducible");
    assert!(
        dockerfile.contains(&format!("FROM rust:{channel}-")),
        "Dockerfile.reproducible must build on rust:{channel}"
    );

    for workflow in ["ci.yml", "release.yml", "integration.yml"] {
        let text = repo_file(&format!(".github/workflows/{workflow}"));
        assert!(
            !text.contains("# stable"),
            "{workflow} must install the toolchain from rust-toolchain.toml, not `stable`"
        );
        assert!(
            text.contains("rustup toolchain install\n"),
            "{workflow} must run `rustup toolchain install` to read rust-toolchain.toml"
        );
    }
}

// ==================== #442: generated ABI ====================

#[test]
fn committed_abi_lists_every_entrypoint() {
    let abi = repo_file("contract/contract-interface.json");
    for name in entrypoints() {
        assert!(
            abi.contains(&format!("\"name\": \"{name}\"")),
            "contract/contract-interface.json is missing `{name}`; run ./scripts/generate-abi.sh"
        );
    }
    let summary = repo_file("contract/interface-summary.txt");
    assert!(summary.contains("fn initialize("));
}

#[test]
fn readme_links_abi_and_ci_checks_for_drift() {
    assert!(repo_file("README.md").contains("(contract/contract-interface.json)"));
    assert!(repo_file(".github/workflows/ci.yml").contains("./scripts/generate-abi.sh --check"));
}

// ==================== #441: generated TypeScript bindings ====================

#[test]
fn committed_bindings_are_the_generated_client() {
    let client = repo_file("bindings/ts/src/index.ts");
    assert!(
        client.contains("export class Client extends ContractClient"),
        "bindings/ts/src/index.ts must be the generated client, not a stub"
    );
    for name in entrypoints() {
        assert!(
            client.contains(&format!("\n  {name}: (")),
            "bindings/ts is missing `{name}`; run ./scripts/generate-bindings.sh"
        );
    }
    // `private` is reserved in strict-mode TS; the generator's raw output
    // destructures it and fails `tsc` unless generate-bindings.sh patches it.
    assert!(!client.contains("private}: {"));

    let package = repo_file("bindings/ts/package.json");
    assert!(package.contains("\"@stellar/stellar-sdk\""));
    assert!(package.contains("\"name\": \"ourdao-dao\""));
}

#[test]
fn ci_checks_bindings_for_drift() {
    assert!(
        repo_file(".github/workflows/ci.yml").contains("./scripts/generate-bindings.sh --check")
    );
}

// ==================== #445: release docs ====================

#[test]
fn deployments_states_no_tag_and_cross_links_changelog() {
    let deployments = repo_file("DEPLOYMENTS.md");
    assert!(
        !deployments.contains("Initial release candidate"),
        "DEPLOYMENTS.md must not present v1.0.0 as a near-term release candidate"
    );
    let history = deployments
        .split("## Release History")
        .nth(1)
        .expect("DEPLOYMENTS.md has a Release History section");
    assert!(history.contains("No version has been tagged yet"));
    assert!(history.contains("[CHANGELOG.md](CHANGELOG.md)"));

    assert!(
        repo_file("CHANGELOG.md").contains("[DEPLOYMENTS.md](DEPLOYMENTS.md)"),
        "CHANGELOG.md must point readers to DEPLOYMENTS.md for what is deployed"
    );
}
