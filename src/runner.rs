use anyhow::{Context as AnyhowContext, Result};
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

use crate::config::{ClawConfig, ReceiverType};

/// Creates a PromptReceiver based on the provided configuration.
///
/// This factory function instantiates the appropriate receiver implementation
/// based on the configuration's receiver_type. If no receiver_type is specified,
/// it defaults to Generic for backward compatibility.
///
/// # Arguments
/// * `config` - The claw configuration containing receiver settings
///
/// # Returns
/// A boxed trait object implementing PromptReceiver
pub fn create_receiver(config: &ClawConfig) -> Box<dyn PromptReceiver> {
    let receiver_type = config.receiver_type.clone().unwrap_or(ReceiverType::Generic);

    match receiver_type {
        ReceiverType::Generic => Box::new(GenericReceiver::new(
            config.llm_command.clone(),
            config.prompt_arg_template.clone(),
        )),
        ReceiverType::ClaudeCli => {
            Box::new(ClaudeCliReceiver::new(config.prompt_arg_template.clone()))
        }
    }
}

/// Defines the contract for sending rendered prompts to different targets.
///
/// This trait abstracts the delivery mechanism for prompts, allowing
/// implementations to use whatever method suits their needs: command-line
/// arguments, stdin piping, IPC, API calls, etc.
pub trait PromptReceiver {
    /// Sends a rendered prompt to the target system.
    ///
    /// Implementations are responsible for:
    /// - Choosing the appropriate delivery mechanism (stdin, args, IPC, API, etc.)
    /// - Handling all communication details
    /// - Reporting errors clearly
    ///
    /// # Arguments
    /// * `prompt` - The fully rendered prompt string to send
    ///
    /// # Returns
    /// * `Ok(())` on successful delivery
    /// * `Err` on any failure with a descriptive error message
    fn send_prompt(&self, prompt: &str) -> Result<()>;

    /// Returns a human-readable name for this receiver type.
    ///
    /// Used for logging and error messages.
    fn name(&self) -> &str;
}

/// Generic receiver that executes arbitrary CLI commands.
///
/// Supports two modes of operation:
/// 1. **Argument mode**: If `prompt_arg_template` contains `{{prompt}}`,
///    the prompt is passed as a command-line argument.
/// 2. **Stdin mode**: If `{{prompt}}` is NOT present in the template,
///    the prompt is piped to the command's stdin.
pub struct GenericReceiver {
    llm_command: String,
    prompt_arg_template: String,
}

impl GenericReceiver {
    /// Creates a new GenericReceiver with the specified command and template.
    pub fn new(llm_command: String, prompt_arg_template: String) -> Self {
        Self {
            llm_command,
            prompt_arg_template,
        }
    }

    /// Sends the prompt via command-line arguments (when {{prompt}} is in template).
    fn send_via_argument(&self, prompt: &str) -> Result<()> {
        // Find the full path to the executable
        let llm_executable = which::which(&self.llm_command).with_context(|| {
            format!(
                "LLM command '{}' not found in your PATH. Please make sure it's installed and accessible.",
                self.llm_command
            )
        })?;

        // Parse the argument template string into a vector of arguments
        let template_args = shlex::split(&self.prompt_arg_template)
            .context("Could not parse 'prompt_arg_template' from your config.")?;

        // Build the command
        let mut command = Command::new(&llm_executable);
        for arg in template_args {
            // Substitute the placeholder with the real prompt
            if arg.contains("{{prompt}}") {
                command.arg(arg.replace("{{prompt}}", prompt));
            } else {
                command.arg(arg);
            }
        }

        // Run the command interactively
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

    /// Sends the prompt via stdin (when {{prompt}} is NOT in template).
    fn send_via_stdin(&self, prompt: &str) -> Result<()> {
        // Find the full path to the executable
        let llm_executable = which::which(&self.llm_command).with_context(|| {
            format!(
                "LLM command '{}' not found in your PATH. Please make sure it's installed and accessible.",
                self.llm_command
            )
        })?;

        // Parse the argument template (for non-prompt flags)
        let template_args = shlex::split(&self.prompt_arg_template)
            .context("Could not parse 'prompt_arg_template' from your config.")?;

        // Build the command with stdin piped
        let mut child = Command::new(&llm_executable)
            .args(&template_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to spawn LLM command: '{}'",
                    llm_executable.display()
                )
            })?;

        // Write prompt to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).with_context(|| {
                format!(
                    "Failed to pass prompt to LLM via stdin. Check if '{}' supports stdin input, or try using {{{{prompt}}}} in prompt_arg_template.",
                    self.llm_command
                )
            })?;
            // stdin is automatically closed when dropped
        }

        // Wait for the command to complete
        let status = child.wait().with_context(|| {
            format!(
                "Failed to wait for LLM command: '{}'",
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
}

impl PromptReceiver for GenericReceiver {
    fn send_prompt(&self, prompt: &str) -> Result<()> {
        if self.prompt_arg_template.contains("{{prompt}}") {
            // Argument-based approach
            self.send_via_argument(prompt)
        } else {
            // Stdin-based approach
            self.send_via_stdin(prompt)
        }
    }

    fn name(&self) -> &str {
        "Generic"
    }
}

/// Convenience receiver for the Claude CLI.
///
/// This receiver hardcodes "claude" as the command and ignores the
/// `llm_command` config field. Otherwise, it behaves identically to
/// GenericReceiver, supporting both stdin and argument-based modes.
pub struct ClaudeCliReceiver {
    prompt_arg_template: String,
}

impl ClaudeCliReceiver {
    /// Creates a new ClaudeCliReceiver with the specified template.
    pub fn new(prompt_arg_template: String) -> Self {
        Self {
            prompt_arg_template,
        }
    }
}

impl PromptReceiver for ClaudeCliReceiver {
    fn send_prompt(&self, prompt: &str) -> Result<()> {
        // Delegate to GenericReceiver with hardcoded "claude" command
        let generic = GenericReceiver::new("claude".to_string(), self.prompt_arg_template.clone());
        generic.send_prompt(prompt)
    }

    fn name(&self) -> &str {
        "ClaudeCli"
    }
}

/// Checks if the prompt is large and using {{prompt}} substitution,
/// and displays a migration warning if appropriate.
///
/// This helps users understand they can avoid shell argument length limits
/// by switching to stdin mode.
pub fn check_prompt_size_warning(prompt: &str, template: &str) {
    const MB: usize = 1024 * 1024;
    if template.contains("{{prompt}}") && prompt.len() > MB {
        eprintln!(
            "⚠️  Warning: Your prompt is over 1MB. Consider removing {{{{prompt}}}} from \
             prompt_arg_template to use stdin for better handling of large contexts."
        );
    }
}

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
