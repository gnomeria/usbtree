use std::process::Command;

#[test]
fn test_cli_dump_runs_successfully() {
    // Cargo automatically sets this environment variable for integration tests
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    let output = Command::new(bin_path)
        .arg("--dump")
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(
        output.status.success(),
        "Binary should exit successfully when running --dump"
    );
    // Note: We don't assert that stdout is not empty here, because CI runners
    // (like GitHub Actions) may legitimately have zero USB devices.
}

#[test]
fn test_cli_version_matches_cargo() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    let output = Command::new(bin_path)
        .arg("--version")
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(output.status.success(), "--version should exit successfully");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        format!("usbtree {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_cli_help_lists_upgrade() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    let output = Command::new(bin_path)
        .arg("--help")
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(output.status.success(), "--help should exit successfully");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--upgrade"), "help should list --upgrade");
}

#[test]
fn test_cli_unknown_flag_errors() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    let output = Command::new(bin_path)
        .arg("--verison")
        .output()
        .expect("Failed to execute usbtree binary");

    assert_eq!(output.status.code(), Some(2), "unknown flag should exit 2");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--verison"), "error should name the bad flag");
}

#[test]
fn test_cli_demo_dump_has_output() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    let output = Command::new(bin_path)
        .arg("--demo")
        .arg("--dump")
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(
        output.status.success(),
        "Binary should exit successfully with --demo --dump"
    );
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Demo dump output should not be empty");
}
