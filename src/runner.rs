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

pub fn run_pass_through(config: &ClawConfig) -> Result<()> {
    let llm_executable = which::which(&config.llm_command).with_context(|| {
        format!(
            "LLM command '{}' not found in your PATH. Please make sure it's installed and accessible.",
            config.llm_command
        )
    })?;

    let mut command = Command::new(&llm_executable);

    let status = command.status().with_context(|| {
        format!(
            "Failed to execute LLM command: '{}'",
            llm_executable.display()
        )
    })?;

    if !status.success() {
        // Since this is a direct pass-through, we don't bail with an error,
        // as the user might have intentionally exited with a non-zero status.
        // We just exit our own process with the same code.
        if let Some(code) = status.code() {
            std::process::exit(code);
        }
    }

    Ok(())
}
