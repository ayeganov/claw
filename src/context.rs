use anyhow::Result;
use content_inspector::{ContentType, inspect};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use termtree::Tree;

use crate::config::ErrorHandlingMode;

/// Configuration for context file discovery and processing.
#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub paths: Vec<PathBuf>,
    pub recurse_depth: Option<usize>,
    pub max_file_size_kb: u64,
    pub max_files_per_directory: usize,
    pub error_handling_mode: ErrorHandlingMode,
    pub excluded_directories: Vec<String>,
    pub excluded_extensions: Vec<String>,
}

/// Represents a discovered file with metadata.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub size: u64,
    pub relative_path: PathBuf,
}

/// The content of a successfully read file.
#[derive(Debug, Clone)]
pub struct FileContent {
    #[allow(dead_code)]
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub content: String,
}

/// Errors that can occur during context processing.
#[derive(Debug, Clone)]
pub enum ContextError {
    #[allow(dead_code)]
    FileNotFound(PathBuf),
    PermissionDenied(PathBuf),
    FileTooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },
    TooManyFiles {
        directory: PathBuf,
        count: usize,
        limit: usize,
    },
    #[allow(dead_code)]
    BinaryFile(PathBuf),
    Utf8Error(PathBuf),
    IoError {
        path: PathBuf,
        error: String,
    },
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextError::FileNotFound(path) => {
                write!(f, "File not found: {}", path.display())
            }
            ContextError::PermissionDenied(path) => {
                write!(f, "Permission denied: {}", path.display())
            }
            ContextError::FileTooLarge { path, size, limit } => {
                write!(
                    f,
                    "File too large: {} ({} KB exceeds limit of {} KB)",
                    path.display(),
                    size,
                    limit
                )
            }
            ContextError::TooManyFiles {
                directory,
                count,
                limit,
            } => {
                write!(
                    f,
                    "Too many files in directory: {} ({} files exceeds limit of {})",
                    directory.display(),
                    count,
                    limit
                )
            }
            ContextError::BinaryFile(path) => {
                write!(f, "Binary file skipped: {}", path.display())
            }
            ContextError::Utf8Error(path) => {
                write!(f, "UTF-8 decoding error: {}", path.display())
            }
            ContextError::IoError { path, error } => {
                write!(f, "I/O error reading {}: {}", path.display(), error)
            }
        }
    }
}

/// Result of context processing, including files, errors, and warnings.
#[derive(Debug)]
pub struct ContextResult {
    pub files: Vec<FileContent>,
    pub errors: Vec<ContextError>,
    pub warnings: Vec<String>,
}

/// Discovers files from the given paths, applying recursion and filtering rules.
pub fn discover_files(config: &ContextConfig) -> Result<Vec<DiscoveredFile>> {
    let mut discovered = Vec::new();
    let cwd = std::env::current_dir()?;

    for path in &config.paths {
        if !path.exists() {
            anyhow::bail!("Path does not exist: {}", path.display());
        }

        if path.is_file() {
            // Single file
            let metadata = fs::metadata(path)?;
            let relative = path.strip_prefix(&cwd).unwrap_or(path);
            discovered.push(DiscoveredFile {
                path: path.clone(),
                size: metadata.len(),
                relative_path: relative.to_path_buf(),
            });
        } else if path.is_dir() {
            // Directory: use walkdir with filters
            let max_depth = config.recurse_depth.map(|d| d + 1);

            let mut builder = WalkBuilder::new(path);
            builder.standard_filters(true); // Respects .gitignore

            if let Some(depth) = max_depth {
                builder.max_depth(Some(depth));
            }

            for entry in builder.build() {
                let entry = entry?;
                let file_path = entry.path();

                // Skip directories
                if file_path.is_dir() {
                    continue;
                }

                // Check if file extension is excluded
                if let Some(ext) = file_path.extension() {
                    let ext_str = ext.to_string_lossy().to_string();
                    if config.excluded_extensions.contains(&ext_str) {
                        continue;
                    }
                }

                // Check if any parent directory is in excluded list
                let mut skip = false;
                for ancestor in file_path.ancestors() {
                    if let Some(name) = ancestor.file_name() {
                        let name_str = name.to_string_lossy().to_string();
                        if config.excluded_directories.contains(&name_str) {
                            skip = true;
                            break;
                        }
                    }
                }
                if skip {
                    continue;
                }

                let metadata = fs::metadata(file_path)?;
                let relative = file_path.strip_prefix(&cwd).unwrap_or(file_path);

                discovered.push(DiscoveredFile {
                    path: file_path.to_path_buf(),
                    size: metadata.len(),
                    relative_path: relative.to_path_buf(),
                });
            }
        }
    }

    Ok(discovered)
}

/// Checks if a file appears to be binary using content inspection.
fn is_binary_file(path: &Path) -> io::Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0u8; 8192];
    let bytes_read = file.read(&mut buffer)?;

    // Use content_inspector to intelligently detect binary vs text
    Ok(matches!(
        inspect(&buffer[..bytes_read]),
        ContentType::BINARY
    ))
}

/// Validates and reads files, applying size limits and binary checks.
pub fn validate_and_read_files(
    files: Vec<DiscoveredFile>,
    config: &ContextConfig,
) -> ContextResult {
    let mut result = ContextResult {
        files: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    // Track file counts per directory
    let mut dir_counts: HashMap<PathBuf, usize> = HashMap::new();

    for file in files {
        // Check file size limit
        let size_kb = file.size / 1024;
        if size_kb > config.max_file_size_kb {
            result.errors.push(ContextError::FileTooLarge {
                path: file.path.clone(),
                size: size_kb,
                limit: config.max_file_size_kb,
            });
            continue;
        }

        // Check directory file count limit
        if let Some(parent) = file.path.parent() {
            let count = dir_counts.entry(parent.to_path_buf()).or_insert(0);
            *count += 1;
            if *count > config.max_files_per_directory {
                result.errors.push(ContextError::TooManyFiles {
                    directory: parent.to_path_buf(),
                    count: *count,
                    limit: config.max_files_per_directory,
                });
                continue;
            }
        }

        // Check if binary file
        match is_binary_file(&file.path) {
            Ok(true) => {
                result
                    .warnings
                    .push(format!("Skipped binary file: {}", file.path.display()));
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    result
                        .errors
                        .push(ContextError::PermissionDenied(file.path.clone()));
                } else {
                    result.errors.push(ContextError::IoError {
                        path: file.path.clone(),
                        error: e.to_string(),
                    });
                }
                continue;
            }
        }

        // Read file content
        match fs::read_to_string(&file.path) {
            Ok(content) => {
                result.files.push(FileContent {
                    path: file.path,
                    relative_path: file.relative_path,
                    content,
                });
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    result
                        .errors
                        .push(ContextError::PermissionDenied(file.path));
                } else if e.kind() == io::ErrorKind::InvalidData {
                    result.errors.push(ContextError::Utf8Error(file.path));
                } else {
                    result.errors.push(ContextError::IoError {
                        path: file.path,
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    result
}

/// Handles errors based on the configured error handling mode.
pub fn handle_errors(result: &ContextResult, mode: &ErrorHandlingMode) -> Result<bool> {
    if result.errors.is_empty() {
        return Ok(true);
    }

    match mode {
        ErrorHandlingMode::Strict => {
            // Fail immediately on any error
            let error_messages: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
            anyhow::bail!(
                "Context processing failed with {} error(s):\n  {}",
                result.errors.len(),
                error_messages.join("\n  ")
            );
        }
        ErrorHandlingMode::Flexible => {
            // Display errors and warnings, then prompt user
            eprintln!("\n⚠️  Context Processing Issues Detected:");
            eprintln!("=====================================");

            if !result.errors.is_empty() {
                eprintln!("\nErrors ({}):", result.errors.len());
                for error in &result.errors {
                    eprintln!("  • {}", error);
                }
            }

            if !result.warnings.is_empty() {
                eprintln!("\nWarnings ({}):", result.warnings.len());
                for warning in &result.warnings {
                    eprintln!("  • {}", warning);
                }
            }

            eprintln!("\nSuccessfully processed {} file(s).", result.files.len());
            eprintln!("\nDo you want to continue with the available files? (y/n): ");

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();

            if input == "y" || input == "yes" {
                Ok(true)
            } else {
                anyhow::bail!("Context processing aborted by user.");
            }
        }
        ErrorHandlingMode::Ignore => {
            // Log warnings and continue
            if !result.warnings.is_empty() {
                eprintln!("\n⚠️  Warnings:");
                for warning in &result.warnings {
                    eprintln!("  • {}", warning);
                }
            }
            if !result.errors.is_empty() {
                eprintln!("\n⚠️  Errors (ignored):");
                for error in &result.errors {
                    eprintln!("  • {}", error);
                }
            }
            Ok(true)
        }
    }
}

/// Formats the context result as markdown for inclusion in the LLM prompt.
pub fn format_context(result: &ContextResult, config: &ContextConfig) -> String {
    // Load the static header template at compile time
    const HEADER_TEMPLATE: &str = include_str!("../prompts/context_header.md");

    let mut output = String::from(HEADER_TEMPLATE);

    // Build the Notes section dynamically
    output.push_str("\n\n## Notes\n");
    output.push_str(&format!(
        "- Maximum file size: {} KB\n",
        config.max_file_size_kb
    ));
    output.push_str(&format!(
        "- Maximum files per directory: {}\n",
        config.max_files_per_directory
    ));
    output.push_str(&format!(
        "- Excluded directories: {}\n",
        config.excluded_directories.join(", ")
    ));
    output.push_str(&format!(
        "- Excluded extensions: {}\n",
        config.excluded_extensions.join(", ")
    ));
    output.push_str(&format!(
        "- Recursion depth: {}\n\n",
        config
            .recurse_depth
            .map_or("unlimited".to_string(), |d| d.to_string())
    ));

    output.push_str("---\n\n");

    // Generate directory tree
    output.push_str("## Directory Structure\n\n");
    output.push_str("```\n");
    output.push_str(&generate_tree(&result.files));
    output.push_str("```\n\n");

    output.push_str("---\n\n");

    // Individual files
    output.push_str("## Files\n\n");
    for file in &result.files {
        output.push_str(&format!("### {}\n\n", file.relative_path.display()));
        output.push_str("```\n");
        output.push_str(&file.content);
        if !file.content.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("```\n\n");
    }

    output
}

/// Generates a tree structure from file paths using termtree.
fn generate_tree(files: &[FileContent]) -> String {
    if files.is_empty() {
        return String::from("(no files)");
    }

    // Build a nested HashMap representing the directory structure
    let mut root: HashMap<String, Node> = HashMap::new();

    for file in files {
        let components: Vec<_> = file.relative_path.components().collect();
        insert_path(&mut root, &components);
    }

    // Convert the HashMap tree to termtree format
    let mut children = Vec::new();
    for (name, node) in root {
        children.push(build_termtree(name, &node));
    }

    // Sort for consistent output
    children.sort_by(|a, b| a.root.cmp(&b.root));

    // Render all trees
    let mut output = String::new();
    for tree in children {
        output.push_str(&tree.to_string());
    }

    output
}

#[derive(Debug)]
enum Node {
    File,
    Directory(HashMap<String, Node>),
}

fn insert_path(tree: &mut HashMap<String, Node>, components: &[std::path::Component]) {
    if components.is_empty() {
        return;
    }

    let name = components[0].as_os_str().to_string_lossy().to_string();

    if components.len() == 1 {
        // This is a file
        tree.insert(name, Node::File);
    } else {
        // This is a directory path
        let subtree = tree
            .entry(name)
            .or_insert_with(|| Node::Directory(HashMap::new()));

        if let Node::Directory(children) = subtree {
            insert_path(children, &components[1..]);
        }
    }
}

fn build_termtree(name: String, node: &Node) -> Tree<String> {
    match node {
        Node::File => Tree::new(name),
        Node::Directory(children) => {
            let mut tree = Tree::new(format!("{}/", name));
            let mut child_trees: Vec<_> = children
                .iter()
                .map(|(child_name, child_node)| build_termtree(child_name.clone(), child_node))
                .collect();

            // Sort children
            child_trees.sort_by(|a, b| a.root.cmp(&b.root));

            for child in child_trees {
                tree.push(child);
            }

            tree
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_binary_detection() {
        // Create temp directory with test files
        let temp_dir = std::env::temp_dir().join("claw_test_binary");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Text file
        let text_file = temp_dir.join("text.txt");
        std::fs::write(&text_file, "Hello, world!").unwrap();
        assert!(!is_binary_file(&text_file).unwrap());

        // Binary file (with null bytes)
        let binary_file = temp_dir.join("binary.bin");
        std::fs::write(&binary_file, &[0u8, 1u8, 2u8, 0u8, 3u8]).unwrap();
        assert!(is_binary_file(&binary_file).unwrap());

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).unwrap();
    }
}
