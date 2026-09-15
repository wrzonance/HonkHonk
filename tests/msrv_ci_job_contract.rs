//! Pins the boundary invariants behind issue #226's `msrv` CI job in
//! `.github/workflows/rust.yml`. Unlike `msrv_matches_dependency_graph.rs`
//! (which checks the declared floor against the *dependency graph*), this
//! suite checks the *workflow file itself*: that the job actually builds
//! against the version Cargo.toml declares, fails loudly instead of
//! silently degrading, and disturbs neither the checked-in lockfile nor the
//! `build` job that branch protection keys off of.
//!
//! The workflow is parsed as text rather than through a YAML crate --
//! matching the existing `Cargo.toml`-parsing convention in this suite --
//! to avoid a new dependency for a handful of scalar/step lookups.

use std::path::Path;
use std::process::{Command, Output};

fn read_workflow() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/rust.yml");
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

/// True for a top-level (2-space indent) `key:` line, i.e. a job header --
/// as opposed to a step field (deeper indent) or a `  # comment` line.
fn is_job_header(line: &str) -> bool {
    let two_space_indent = line.starts_with("  ") && !line.starts_with("   ");
    let trimmed = line.trim_start();
    two_space_indent && !trimmed.starts_with('#') && !trimmed.is_empty() && trimmed.ends_with(':')
}

/// Slices out one job's lines (its header through the line before the next
/// job header), preserving original indentation.
fn job_block_lines<'a>(lines: &[&'a str], job_name: &str) -> Vec<&'a str> {
    let header = format!("  {job_name}:");
    let start = lines
        .iter()
        .position(|line| *line == header)
        .unwrap_or_else(|| panic!("job `{job_name}` not found in rust.yml"));
    let end = lines[(start + 1)..]
        .iter()
        .position(|line| is_job_header(line))
        .map(|offset| start + 1 + offset)
        .unwrap_or(lines.len());
    lines[start..end].to_vec()
}

/// Finds a step by its `- name:` line and returns the index range
/// `[step_start, next_step_start)` within `block`.
fn step_range(block: &[&str], step_name: &str) -> (usize, usize) {
    let marker = format!("- name: {step_name}");
    let start = block
        .iter()
        .position(|line| line.trim_start() == marker)
        .unwrap_or_else(|| panic!("step `{step_name}` not found"));
    let end = block[(start + 1)..]
        .iter()
        .position(|line| {
            let t = line.trim_start();
            t.starts_with("- name:") || t.starts_with("- uses:")
        })
        .map(|offset| start + 1 + offset)
        .unwrap_or(block.len());
    (start, end)
}

fn toolchain_value_for_step<'a>(block: &[&'a str], step_name: &str) -> &'a str {
    let (start, end) = step_range(block, step_name);
    block[start..end]
        .iter()
        .find_map(|line| line.trim_start().strip_prefix("toolchain:"))
        .map(str::trim)
        .unwrap_or_else(|| panic!("step `{step_name}` has no toolchain: field"))
}

fn run_command_for_step<'a>(block: &[&'a str], step_name: &str) -> &'a str {
    let (start, end) = step_range(block, step_name);
    block[start..end]
        .iter()
        .find_map(|line| line.trim_start().strip_prefix("run:"))
        .map(str::trim)
        // A `run: |` block scalar strips to a bare `|`, which is a *shape*
        // change, not a command -- reject it so the panic below names the
        // real problem instead of returning "|" as if it were the command.
        .filter(|command| !command.is_empty() && *command != "|")
        .unwrap_or_else(|| panic!("step `{step_name}` has no single-line `run:` command"))
}

/// Extracts the literal shell script under a `run: |` block scalar, keeping
/// each line's original indentation (harmless for plain sequential bash).
fn extract_run_block(block: &[&str], step_name: &str) -> String {
    let (start, end) = step_range(block, step_name);
    let run_idx = block[start..end]
        .iter()
        .position(|line| line.trim_start() == "run: |")
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("step `{step_name}` has no `run: |` block"));
    let run_indent = block[run_idx].len() - block[run_idx].trim_start().len();
    block[(run_idx + 1)..end]
        .iter()
        .take_while(|line| {
            line.trim().is_empty() || (line.len() - line.trim_start().len()) > run_indent
        })
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_fixture_cargo_toml(dir: &Path, rust_version_line: Option<&str>) {
    let mut contents = String::from("[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n");
    if let Some(line) = rust_version_line {
        contents.push_str(line);
        contents.push('\n');
    }
    std::fs::write(dir.join("Cargo.toml"), contents).expect("failed to write fixture Cargo.toml");
}

fn run_extracted_script(script: &str, cwd: &Path, github_output: &Path) -> Output {
    Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(cwd)
        .env("GITHUB_OUTPUT", github_output)
        .output()
        .expect("failed to spawn bash for the extracted read-msrv script")
}

fn output_has_version(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|contents| contents.contains("version="))
        .unwrap_or(false)
}

/// Invariant: the msrv job's toolchain is always derived from Cargo.toml's
/// `rust-version` at run time, never hardcoded in the workflow.
#[test]
fn msrv_toolchain_is_derived_from_cargo_toml_not_hardcoded() {
    let text = read_workflow();
    let lines: Vec<&str> = text.lines().collect();
    let block = job_block_lines(&lines, "msrv");

    let toolchain = toolchain_value_for_step(&block, "Install Rust toolchain (MSRV)");

    assert_eq!(
        toolchain, "${{ steps.read-msrv.outputs.version }}",
        "msrv job must install the toolchain named by read-msrv's output, not a literal version"
    );
}

/// Invariant: the read-msrv step never lets a missing or malformed
/// `rust-version` line silently degrade to an empty/default toolchain
/// input -- it must fail the job loudly instead.
#[test]
fn read_msrv_step_fails_loudly_on_missing_or_malformed_version() {
    let text = read_workflow();
    let lines: Vec<&str> = text.lines().collect();
    let block = job_block_lines(&lines, "msrv");
    let script = extract_run_block(&block, "Read declared MSRV from Cargo.toml");
    assert!(!script.is_empty(), "read-msrv script must not be empty");

    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let output_path = tmp.path().join("github_output");

    write_fixture_cargo_toml(tmp.path(), None);
    let missing = run_extracted_script(&script, tmp.path(), &output_path);
    assert!(
        !missing.status.success(),
        "must exit non-zero when rust-version is absent"
    );
    assert!(
        !output_has_version(&output_path),
        "must never emit a version output when rust-version is absent"
    );

    let _ = std::fs::remove_file(&output_path);
    write_fixture_cargo_toml(tmp.path(), Some("rust-version = \"not-a-version\""));
    let malformed = run_extracted_script(&script, tmp.path(), &output_path);
    assert!(
        !malformed.status.success(),
        "must exit non-zero on a malformed rust-version"
    );
    assert!(
        !output_has_version(&output_path),
        "must never emit a version output for a malformed rust-version"
    );

    let _ = std::fs::remove_file(&output_path);
    write_fixture_cargo_toml(tmp.path(), Some("rust-version = \"1.89\""));
    let ok = run_extracted_script(&script, tmp.path(), &output_path);
    assert!(
        ok.status.success(),
        "a well-formed rust-version must succeed"
    );
    let emitted =
        std::fs::read_to_string(&output_path).expect("GITHUB_OUTPUT must exist on success");
    assert_eq!(emitted.trim(), "version=1.89");
}

/// Invariant: the read-msrv step reads `[package].rust-version` specifically,
/// never some other table's key. A manifest that grows a `[workspace.package]`
/// table above `[package]` must not silently install a toolchain that
/// `[package]` does not declare -- the failure would be invisible, since the
/// wrong value still satisfies the version regex.
#[test]
fn read_msrv_step_reads_only_the_package_table() {
    let text = read_workflow();
    let lines: Vec<&str> = text.lines().collect();
    let block = job_block_lines(&lines, "msrv");
    let script = extract_run_block(&block, "Read declared MSRV from Cargo.toml");

    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let output_path = tmp.path().join("github_output");
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[workspace.package]\nrust-version = \"1.70\"\n\n\
         [package]\nname = \"fixture\"\nversion = \"0.1.0\"\nrust-version = \"1.89\"\n",
    )
    .expect("failed to write fixture Cargo.toml");

    let output = run_extracted_script(&script, tmp.path(), &output_path);
    assert!(
        output.status.success(),
        "a manifest with both tables must still resolve a version"
    );
    let emitted =
        std::fs::read_to_string(&output_path).expect("GITHUB_OUTPUT must exist on success");
    assert_eq!(
        emitted.trim(),
        "version=1.89",
        "must read [package].rust-version, not [workspace.package]'s"
    );
}

/// Invariant: the existing `build` job is untouched, so branch protection's
/// required status check (which matches on job id and/or display name)
/// keeps matching.
#[test]
fn build_job_identity_is_untouched() {
    let text = read_workflow();
    let lines: Vec<&str> = text.lines().collect();
    // Asserted before `job_block_lines`, which panics on a missing header:
    // a rename must fail with the consequence (branch protection stops
    // matching), not with a generic "job not found" from the helper.
    assert!(
        lines.contains(&"  build:"),
        "the `build` job id must not be renamed"
    );
    let block = job_block_lines(&lines, "build");

    // Searched within the job block rather than pinned to `block[1]`: inserting
    // a `needs:`/`permissions:` key or a comment above `name:` changes neither
    // identity, and must not fail this test.
    assert!(
        block
            .iter()
            .any(|line| line.trim_end() == "    name: Build (release)"),
        "the `build` job's display name must not change, or branch protection stops matching"
    );
}

/// Invariant: the msrv job's check step carries `--locked`, so the MSRV
/// toolchain can never re-resolve `Cargo.lock` (which would silently require
/// regenerating the Flatpak `cargo-sources.json`).
///
/// This asserts the workflow *text* and deliberately does not execute the
/// extracted command. Running a nested `cargo check` here would be
/// non-hermetic -- it omits `--offline`, so a cold registry cache or a
/// network-sandboxed build (the AUR `honkhonk` PKGBUILD runs
/// `cargo test --frozen --release`) would fail on network access rather than
/// on any real contract violation. It would also run under the *host*
/// toolchain rather than the MSRV one, so it could not evidence the MSRV
/// contract regardless. See `msrv_matches_dependency_graph.rs`, which pins
/// `--offline` on its own `cargo metadata` call for the same reason.
#[test]
fn msrv_cargo_check_step_is_locked() {
    let text = read_workflow();
    let lines: Vec<&str> = text.lines().collect();
    let block = job_block_lines(&lines, "msrv");
    let command = run_command_for_step(&block, "cargo check (MSRV)");
    assert_eq!(
        command, "cargo check --locked",
        "the msrv job's check step must run exactly `cargo check --locked`"
    );
}
