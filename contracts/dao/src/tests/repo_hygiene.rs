//! Repository hygiene checks that read files from the checkout rather than
//! exercising the contract: the ADR index (#443), dead code that must stay
//! deleted (#446, #448), and the integration workflow's script references
//! (#444).

extern crate std;

use std::fs;
use std::string::String;
use std::vec::Vec;

const REPO_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn read(rel: &str) -> String {
    let path = std::format!("{REPO_ROOT}/{rel}");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path} must be readable: {e}"))
}

fn exists(rel: &str) -> bool {
    std::path::Path::new(&std::format!("{REPO_ROOT}/{rel}")).exists()
}

/// `0001` .. `NNNN` numbers of the ADR files present in `docs/adr/`.
fn adr_numbers_on_disk() -> Vec<u32> {
    let mut nums: Vec<u32> = fs::read_dir(std::format!("{REPO_ROOT}/docs/adr"))
        .expect("docs/adr must be readable")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            let (prefix, _) = name.split_once('-')?;
            (prefix.len() == 4).then(|| prefix.parse().ok()).flatten()
        })
        .collect();
    nums.sort_unstable();
    nums
}

/// The first cell of every table row in the ADR README that names an ADR
/// number, whether as a link (`[0004](...)`) or a bare number (`0002`).
fn adr_numbers_in_readme_tables(readme: &str) -> Vec<u32> {
    readme
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|'))
        .filter_map(|l| {
            let cell = l.trim_matches('|').split('|').next()?.trim();
            let digits = cell.trim_start_matches('[');
            digits.get(..4)?.parse().ok()
        })
        .collect()
}

// ==================== issue #443: ADR numbering gaps ====================
#[test]
fn adr_readme_accounts_for_every_number_up_to_the_highest() {
    let on_disk = adr_numbers_on_disk();
    let max = *on_disk.last().expect("at least one ADR exists");
    let readme = read("docs/adr/README.md");
    let listed = adr_numbers_in_readme_tables(&readme);

    for n in 1..=max {
        assert!(
            listed.contains(&n),
            "ADR {n:04} is neither in the index nor in the unused-numbers table of docs/adr/README.md"
        );
    }
    for n in &on_disk {
        assert!(
            readme.contains(&std::format!("[{n:04}](./{n:04}-")),
            "ADR {n:04} exists on disk but is not linked from the index"
        );
    }
}

#[test]
fn adr_readme_documents_append_only_numbering() {
    let readme = read("docs/adr/README.md");
    assert!(readme.contains("append-only"));
    assert!(readme.contains("Never reuse a skipped number"));
    assert!(readme.contains("Never renumber"));
}

// ==================== issue #446: orphaned repay_partial ====================
#[test]
fn orphaned_repay_partial_helper_stays_deleted() {
    let loans = read("contracts/dao/src/loans.rs");
    assert!(!loans.contains("fn repay_partial("));
    assert!(!loans.contains("\"loan_partial_repaid\""));
    assert!(!loans.contains("allow(dead_code"));
}

// ==================== issue #448: dead TotalContributions key ====================
#[test]
fn dead_total_contributions_key_stays_deleted() {
    let storage = read("contracts/dao/src/storage.rs");
    assert!(!storage.contains("TotalContributions"));
    assert!(!storage.contains("fn get_total_contributions"));
    assert!(!storage.contains("fn set_total_contributions"));
    assert!(!storage.contains("allow(dead_code"));
}

// ==================== issue #444: integration workflow scripts ====================
/// Workflow triggers live in the top-level `on:` block, up to the next
/// unindented key.
fn workflow_triggers(workflow: &str) -> String {
    let mut out = String::new();
    let mut in_on = false;
    for line in workflow.lines() {
        if line.starts_with("on:") {
            in_on = true;
            continue;
        }
        if in_on {
            if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('#') {
                break;
            }
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[test]
fn integration_workflow_only_runs_automatically_when_its_scripts_exist() {
    let workflow = read(".github/workflows/integration.yml");
    let triggers = workflow_triggers(&workflow);
    let runs_automatically = triggers.contains("push:") || triggers.contains("pull_request:");

    let missing: Vec<&str> = workflow
        .split("./scripts/")
        .skip(1)
        .filter_map(|rest| rest.split_whitespace().next())
        .filter(|script| !exists(&std::format!("scripts/{script}")))
        .collect();

    assert!(
        !runs_automatically || missing.is_empty(),
        "integration.yml runs on push/PR but calls missing scripts: {missing:?}"
    );
    if !missing.is_empty() {
        let readme = read("README.md");
        assert!(
            workflow.contains("DISABLED (#444)") && readme.contains("#444"),
            "a disabled integration workflow must say why in both the workflow and README.md"
        );
    }
}
