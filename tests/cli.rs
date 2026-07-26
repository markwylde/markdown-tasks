use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn help_and_version_use_the_mdt_binary_name() {
    cargo_bin_cmd!("mdt")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("A fast Markdown task report"))
        .stdout(predicate::str::contains("Usage: mdt"));

    cargo_bin_cmd!("mdt")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("mdt 0.1.0"));
}

#[test]
fn default_and_explicit_paths_print_reports_and_exit() {
    let temporary = tempdir().unwrap();
    fs::write(
        temporary.path().join("plan.md"),
        "# Plan\n\n- [x] done\n- [ ] remaining\n",
    )
    .unwrap();

    cargo_bin_cmd!("mdt")
        .current_dir(temporary.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("50% complete"))
        .stdout(predicate::str::contains("[x] done"))
        .stdout(predicate::str::contains("[ ] remaining"));

    cargo_bin_cmd!("mdt")
        .arg(temporary.path().join("plan.md"))
        .assert()
        .success()
        .stdout(predicate::str::contains("plan.md  1/2  50%"));
}

#[test]
fn argument_and_target_failures_use_the_documented_exit_codes() {
    cargo_bin_cmd!("mdt")
        .arg("--not-a-real-option")
        .assert()
        .code(2);

    cargo_bin_cmd!("mdt")
        .arg("/definitely/missing/mdt-target")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("target does not exist"));

    let temporary = tempdir().unwrap();
    let unsupported = temporary.path().join("notes.txt");
    fs::write(&unsupported, "- [ ] not scanned").unwrap();
    cargo_bin_cmd!("mdt")
        .arg(unsupported)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unsupported Markdown"));
}

#[test]
fn tui_rejects_piped_stdio_before_mutating_the_terminal() {
    let temporary = tempdir().unwrap();
    fs::write(temporary.path().join("plan.md"), "- [ ] task").unwrap();

    cargo_bin_cmd!("mdt")
        .args(["--tui"])
        .arg(temporary.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "interactive mode requires both stdin and stdout",
        ))
        .stdout(predicate::str::is_empty());
}

#[test]
fn redirected_never_color_output_has_no_terminal_control_bytes() {
    let temporary = tempdir().unwrap();
    fs::write(temporary.path().join("plan.md"), "- [x] done").unwrap();

    let output = cargo_bin_cmd!("mdt")
        .args(["--color", "never"])
        .arg(temporary.path())
        .write_stdin("input that must not be read")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    assert!(output.stdout.ends_with(b"\n"));
    assert!(output.stderr.is_empty());
}
