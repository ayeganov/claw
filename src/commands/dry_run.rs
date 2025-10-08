use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Handles the dry-run command by rendering a goal's prompt without executing the LLM.
///
/// # Arguments
/// * `goal_name` - Name of the goal to render
/// * `output_file` - Optional file path to write the rendered prompt
/// * `rendered_prompt` - The fully rendered prompt string
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` with appropriate context on failure
pub fn handle_dry_run_command(
    output_file: Option<&PathBuf>,
    rendered_prompt: &str,
) -> Result<()> {
    output_prompt(rendered_prompt, output_file)?;
    Ok(())
}

/// Outputs the rendered prompt either to stdout or to a file.
///
/// # Arguments
/// * `prompt` - The rendered prompt string to output
/// * `output_file` - Optional file path; if None, outputs to stdout
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` with file write error context if file output fails
fn output_prompt(prompt: &str, output_file: Option<&PathBuf>) -> Result<()> {
    match output_file {
        None => {
            // Write to stdout (no trailing newline to match exact LLM input)
            print!("{}", prompt);
            Ok(())
        }
        Some(path) => {
            // Write to file
            fs::write(path, prompt.as_bytes()).with_context(|| {
                format!("Failed to write dry run output to {}", path.display())
            })?;

            // Print confirmation to stdout
            println!("Dry run output written to {}", path.display());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_output_prompt_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_output.txt");
        let test_prompt = "This is a test prompt\nWith multiple lines.";

        let result = output_prompt(test_prompt, Some(&output_path));
        assert!(result.is_ok());

        // Verify file contents
        let file_contents = fs::read_to_string(&output_path).unwrap();
        assert_eq!(file_contents, test_prompt);
    }

    #[test]
    fn test_output_prompt_overwrites_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("existing_file.txt");

        // Create file with initial content
        fs::write(&output_path, "Old content").unwrap();

        // Overwrite with new content
        let new_prompt = "New prompt content";
        let result = output_prompt(new_prompt, Some(&output_path));
        assert!(result.is_ok());

        // Verify only new content exists
        let file_contents = fs::read_to_string(&output_path).unwrap();
        assert_eq!(file_contents, new_prompt);
        assert!(!file_contents.contains("Old content"));
    }

    #[test]
    fn test_output_prompt_handles_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("unicode_test.txt");
        let test_prompt = "Test with unicode: 你好世界 🚀 café";

        let result = output_prompt(test_prompt, Some(&output_path));
        assert!(result.is_ok());

        // Verify unicode is preserved
        let file_contents = fs::read_to_string(&output_path).unwrap();
        assert_eq!(file_contents, test_prompt);
    }

    #[test]
    fn test_output_prompt_handles_empty_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("empty_test.txt");
        let empty_prompt = "";

        let result = output_prompt(empty_prompt, Some(&output_path));
        assert!(result.is_ok());

        // Verify empty file created
        let file_contents = fs::read_to_string(&output_path).unwrap();
        assert_eq!(file_contents, "");
    }

    #[test]
    fn test_output_prompt_handles_large_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("large_test.txt");
        // Create a large prompt (>1MB)
        let large_prompt = "x".repeat(2 * 1024 * 1024);

        let result = output_prompt(&large_prompt, Some(&output_path));
        assert!(result.is_ok());

        // Verify size
        let metadata = fs::metadata(&output_path).unwrap();
        assert_eq!(metadata.len(), 2 * 1024 * 1024);
    }
}
