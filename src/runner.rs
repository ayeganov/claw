use anyhow::{Context as AnyhowContext, Result};
use std::collections::HashMap;
use std::process::Command;

use crate::config::ClawConfig;

/// Executes all shell commands defined in the `context_scripts` map.
///
/// Returns a HashMap where the key is the script name and the value is
/// the captured standard output of the script. If any script fails,
/// it returns an error containing the script's stderr.
pub fn execute_context_scripts(
    scripts: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let mut outputs = HashMap::new();

    for (name, command_str) in scripts {
        // We use `sh -c` to ensure that shell features like pipes and globbing
        // work as expected, which is common for dev tools.
        let output = Command::new("sh")
            .arg("-c")
            .arg(command_str)
            .output()
            .with_context(|| format!("Failed to execute context script '{}'", name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Context script '{}' (`{}`) failed with status {}:\n{}",
                name,
                command_str,
                output.status,
                stderr
            );
        }

        let stdout = String::from_utf8(output.stdout)
            .with_context(|| format!("Script output for '{}' was not valid UTF-8", name))?;

        outputs.insert(name.clone(), stdout.trim().to_string());
    }

    Ok(outputs)
}

pub fn run_llm(config: &ClawConfig, prompt: &str) -> Result<()> {
    // 1. Find the full path to the executable.
    let llm_executable = which::which(&config.llm_command).with_context(|| {
        format!(
            "LLM command '{}' not found in your PATH. Please make sure it's installed and accessible.",
            config.llm_command
        )
    })?;

    // 2. Parse the argument template string into a vector of arguments.
    // `shlex` handles spaces and quotes correctly.
    let template_args = shlex::split(&config.prompt_arg_template)
        .context("Could not parse 'prompt_arg_template' from your config.")?;

    // 3. Build the command.
    let mut command = Command::new(&llm_executable);
    for arg in template_args {
        // 4. Substitute the placeholder with the real prompt.
        if arg.contains("{{prompt}}") {
            command.arg(arg.replace("{{prompt}}", prompt));
        } else {
            command.arg(arg);
        }
    }

    // 5. Run the command interactively.
    let status = command.status().with_context(|| {
        format!(
            "Failed to execute LLM command: '{}'",
            llm_executable.display()
        )
    })?;

    if !status.success() {
        anyhow::bail!(
            "LLM command '{}' exited with non-zero status: {}",
            llm_executable.display(),
            status
        );
    }

    Ok(())
}

//pub fn run_llm(config: &ClawConfig, prompt: &str) -> Result<()> {
//    // --- THE FIX ---
//    // The script must use "$1" because we are providing a dummy value for "$0".
//    let template_with_placeholder = config.prompt_arg_template.replace("{{prompt}}", "$1");
//    // ---------------
//
//    let script = format!("{} {}", config.llm_command, template_with_placeholder);
//
//    let mut command = Command::new("sh");
//
//    // For the shell command `sh -c script arg0 arg1 ...`:
//    // `arg0` is assigned to `$0` inside the script.
//    // `arg1` is assigned to `$1` inside the script.
//    // We provide a dummy name for $0 ("claw-script") and pass the real prompt as the value for $1.
//    command
//        .arg("-c")
//        .arg(&script)
//        .arg("claw-script")
//        .arg(prompt);
//
//    // --- DEBUGGING ---
//    // This will print the exact components before execution.
//    println!("\n--- CLAW DEBUG ---");
//    println!("Final script sent to sh -c: '{}'", script);
//    println!("Argument for $0 (name):   'claw-script'");
//    println!("Argument for $1 (prompt): '{}'", prompt.trim());
//    println!("--------------------\n");
//    // -----------------
//
//    let status = command
//        .status()
//        .with_context(|| format!("Failed to execute LLM command: '{}'", config.llm_command))?;
//
//    if !status.success() {
//        anyhow::bail!(
//            "LLM command '{}' exited with non-zero status: {}",
//            config.llm_command,
//            status
//        );
//    }
//
//    Ok(())
//}

// Executes the configured LLM command interactively.
//
// This function constructs a shell command from the `ClawConfig` and the
// rendered prompt, then executes it, passing through stdin, stdout, and stderr.
// This effectively drops the user into the interactive LLM session.

//pub fn run_llm(config: &ClawConfig, prompt: &str) -> Result<()> {
//    // Replace the placeholder with a shell positional parameter ($1).
//    // This is safer than direct formatting to avoid shell injection.
//    let template_with_placeholder = config.prompt_arg_template.replace("{{prompt}}", "$1");
//
//    // Combine the command and the template into a single script for `sh -c`.
//    let script = format!("{} {}", config.llm_command, template_with_placeholder);
//
//    println!("Executing the script: {}", script);
//    // The `--` argument tells `sh -c` to stop processing its own options
//    // and treat subsequent arguments as positional parameters for the script.
//    let mut command = Command::new("sh");
//    command.arg("-c").arg(script).arg("--").arg(prompt);
//
//    // Use `.status()` instead of `.output()` to run the command interactively.
//    // This connects the command's stdin/stdout/stderr to our own.
//    let status = command
//        .status()
//        .with_context(|| format!("Failed to execute LLM command: '{}'", config.llm_command))?;
//
//    if !status.success() {
//        // We don't have stderr/stdout to print, but we can report the exit code.
//        anyhow::bail!(
//            "LLM command '{}' exited with non-zero status: {}",
//            config.llm_command,
//            status
//        );
//    }
//
//    Ok(())
//}
