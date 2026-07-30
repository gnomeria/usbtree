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

#[test]
fn test_cli_demo_json_has_snapshot_shape() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    let output = Command::new(bin_path)
        .arg("--demo")
        .arg("--json")
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"format\": \"usbtree.snapshot.v1\""));
    assert!(stdout.contains("\"interfaces\""));
    assert!(stdout.contains("\"configurations\""));
}

#[test]
fn test_cli_demo_markdown_has_report_shape() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");

    let output = Command::new(bin_path)
        .arg("--demo")
        .arg("--markdown")
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# usbtree report"));
    assert!(stdout.contains("## 1-2"));
    assert!(stdout.contains("Configuration 1"));
}

#[test]
fn test_cli_demo_snapshot_and_diff() {
    let bin_path = env!("CARGO_BIN_EXE_usbtree");
    let path = std::env::temp_dir().join(format!(
        "usbtree-test-{}-{}.json",
        std::process::id(),
        "snapshot"
    ));

    let output = Command::new(bin_path)
        .arg("--demo")
        .arg("--snapshot")
        .arg(&path)
        .output()
        .expect("Failed to execute usbtree binary");

    assert!(output.status.success());
    assert!(path.exists());

    let output = Command::new(bin_path)
        .arg("--demo")
        .arg("--diff")
        .arg(&path)
        .output()
        .expect("Failed to execute usbtree binary");

    let _ = std::fs::remove_file(&path);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("added: 0, removed: 0, changed: 0"));
}
