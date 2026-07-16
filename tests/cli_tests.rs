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
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Dump output should not be empty");
}

#[test]
fn test_cli_help() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    // The TUI doesn't have clap yet, but it handles invalid arguments or --help gracefully if we add it.
    // For now we just test --demo.
    let output = Command::new(bin_path)
        .arg("--demo")
        .arg("--dump") // Combine with dump so it exits immediately instead of opening TUI
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(
        output.status.success(),
        "Binary should exit successfully with --demo --dump"
    );
}
