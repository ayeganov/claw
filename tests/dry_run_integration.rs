use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper to create a Command for claw
fn claw() -> Command {
    Command::cargo_bin("claw").expect("Failed to find claw binary")
}

#[test]
fn test_dry_run_simple_goal() {
    // Test with the test_goal that exists in .claw/
    claw()
        .args(&["dry-run", "test_goal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("world-class research assistant"));
}

#[test]
fn test_dry_run_with_output_file() {
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("dry_run_output.txt");
    let output_path = output_file.to_str().unwrap();

    claw()
        .args(&["dry-run", "test_goal", "--output", output_path])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Dry run output written to {}",
            output_path
        )));

    // Verify file was created and contains the prompt
    assert!(output_file.exists(), "Output file should exist");
    let contents = fs::read_to_string(&output_file).unwrap();
    assert!(
        contents.contains("world-class research assistant"),
        "File should contain goal prompt"
    );
}

#[test]
fn test_dry_run_with_short_output_flag() {
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("dry_run_short.txt");
    let output_path = output_file.to_str().unwrap();

    claw()
        .args(&["dry-run", "test_goal", "-o", output_path])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Dry run output written to {}",
            output_path
        )));

    // Verify file was created
    assert!(output_file.exists(), "Output file should exist");
}

#[test]
fn test_dry_run_with_parameters() {
    // Use test-params goal which has required parameters
    claw()
        .args(&[
            "dry-run",
            "test-params",
            "--",
            "--scope",
            "authentication",
            "--format",
            "json",
            "--verbose",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("authentication"))
        .stdout(predicate::str::contains("json"))
        .stdout(predicate::str::contains("true"));
}

#[test]
fn test_dry_run_with_file_context() {
    // Use a file from the repo as context
    claw()
        .args(&["dry-run", "test_goal", "--context", "Cargo.toml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cargo.toml"))
        .stdout(predicate::str::contains("[package]"));
}

#[test]
fn test_dry_run_nonexistent_goal() {
    claw()
        .args(&["dry-run", "nonexistent-goal-xyz"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Goal 'nonexistent-goal-xyz' not found"));
}

#[test]
fn test_dry_run_missing_required_parameter() {
    // test-params requires --scope parameter
    claw()
        .args(&["dry-run", "test-params"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("scope"));
}

#[test]
fn test_dry_run_file_overwrite() {
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("overwrite_test.txt");
    let output_path = output_file.to_str().unwrap();

    // Write initial content
    fs::write(&output_file, "Old content that should be replaced").unwrap();

    // Run dry-run to overwrite
    claw()
        .args(&["dry-run", "test_goal", "--output", output_path])
        .assert()
        .success();

    // Verify old content is gone
    let contents = fs::read_to_string(&output_file).unwrap();
    assert!(
        !contents.contains("Old content"),
        "Old content should be overwritten"
    );
    assert!(
        contents.contains("world-class research assistant"),
        "New content should be present"
    );
}

#[test]
fn test_dry_run_with_all_features() {
    // Test combining output file, context, and parameters
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("combined_test.txt");
    let output_path = output_file.to_str().unwrap();

    claw()
        .args(&[
            "dry-run",
            "test-params",
            "--context",
            "Cargo.toml",
            "--output",
            output_path,
            "--",
            "--scope",
            "testing",
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run output written to"));

    // Verify file contains all elements
    let contents = fs::read_to_string(&output_file).unwrap();
    assert!(
        contents.contains("testing"),
        "Should have parameter substitution"
    );
    assert!(
        contents.contains("markdown"),
        "Should have format parameter"
    );
    assert!(contents.contains("Cargo.toml"), "Should have file context");
}

#[test]
fn test_dry_run_stdout_vs_file_output() {
    // Run to stdout
    let stdout_output = claw()
        .args(&["dry-run", "test_goal"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Run to file
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("comparison.txt");
    let output_path = output_file.to_str().unwrap();

    claw()
        .args(&["dry-run", "test_goal", "--output", output_path])
        .assert()
        .success();

    let file_output = fs::read_to_string(&output_file).unwrap();
    let stdout_str = String::from_utf8_lossy(&stdout_output);

    // Compare outputs (they should be identical)
    assert_eq!(
        stdout_str.trim(),
        file_output.trim(),
        "stdout and file output should be identical"
    );
}

#[test]
fn test_dry_run_help() {
    claw()
        .args(&["dry-run", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Render a goal's prompt"))
        .stdout(predicate::str::contains("--output"))
        .stdout(predicate::str::contains("--context"));
}
