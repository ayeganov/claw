use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(&out_dir).ancestors().nth(4).unwrap().to_path_buf();

    let assets_src = PathBuf::from("assets");
    let assets_dest = target_dir.join("assets");

    if assets_src.exists() {
        if assets_dest.exists() {
            let _ = fs::remove_dir_all(&assets_dest);
        }

        let mut options = fs_extra::dir::CopyOptions::new();
        options.overwrite = true;
        options.copy_inside = false;

        if let Err(e) = fs_extra::dir::copy(&assets_src, &target_dir, &options) {
            println!("cargo:warning=Failed to copy assets: {}", e);
        }

        create_test_params_goal(&assets_dest);
        create_local_claw_test_goal(&target_dir);
    }

    println!("cargo:rerun-if-changed=assets");
}

fn get_test_params_yaml() -> &'static str {
    r#"name: Test Parameters Goal
description: A test goal with required and optional parameters

parameters:
  - name: scope
    description: The scope of the operation
    required: true
    type: string
  - name: format
    description: Output format
    required: true
    type: string
  - name: verbose
    description: Enable verbose output
    required: false
    type: boolean
    default: "false"

prompt: |
  Test goal with parameters:
  - Scope: {{ Args.scope }}
  - Format: {{ Args.format }}
  - Verbose: {{ Args.verbose }}
"#
}

fn create_test_params_goal(assets_dir: &PathBuf) {
    let test_params_dir = assets_dir.join("goals").join("test-params");
    if let Err(e) = fs::create_dir_all(&test_params_dir) {
        println!("cargo:warning=Failed to create test-params directory: {}", e);
        return;
    }

    let prompt_file = test_params_dir.join("prompt.yaml");
    if let Err(e) = fs::write(&prompt_file, get_test_params_yaml()) {
        println!("cargo:warning=Failed to write test-params prompt.yaml: {}", e);
    }
}

fn create_local_claw_test_goal(_target_dir: &PathBuf) {
    let local_claw_dir = PathBuf::from(".claw/goals/test-params");
    if let Err(e) = fs::create_dir_all(&local_claw_dir) {
        println!("cargo:warning=Failed to create .claw/goals/test-params: {}", e);
        return;
    }

    let prompt_file = local_claw_dir.join("prompt.yaml");
    if let Err(e) = fs::write(&prompt_file, get_test_params_yaml()) {
        println!("cargo:warning=Failed to write .claw/goals/test-params: {}", e);
    }
}
