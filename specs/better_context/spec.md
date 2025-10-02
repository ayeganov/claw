# Context Management 2.0: File Embedding System

## 1. Overview

### Problem Statement
Currently, claw users have limited ability to pass arbitrary files as context to their LLM goals. While `context_scripts` can execute shell commands to gather context, there's no streamlined, safe, and user-friendly way to directly include file contents from the filesystem. Users need a robust mechanism to:
- Pass individual files or entire directories as context
- Control recursion depth when including directories
- Ensure safety through file size limits and type restrictions
- Handle errors gracefully based on their preferences

### Goals
1. Enable users to pass arbitrary files and directories as additional context to any goal via CLI parameters
2. Provide safe, configurable file inclusion with size limits, type filtering, and directory exclusions
3. Maintain a clean, deterministic format for injecting file contents into LLM prompts
4. Ensure backward compatibility with existing goals while refactoring them to leverage the new system
5. Optimize performance by limiting files per directory and providing intelligent filtering

### Target Users
All claw users who need to pass files as context, including:
- Developers wanting to include specific source files for code review
- Users converting or analyzing media files (video lists, documents, etc.)
- Anyone needing to provide relevant documents to their LLM goal

### Success Criteria
- Users can specify files/directories via `--context` flag for any goal
- Files are automatically read, validated, and formatted into a clean markdown structure
- File size limits and per-directory file count limits are respected
- Configurable error handling modes (strict/flexible/ignore) work as expected
- Existing goals continue to work and can be easily refactored to use the new system

## 2. Requirements

### Functional Requirements

#### FR1: CLI Interface
- **FR1.1**: Add `-c, --context` parameter accepting space-separated list of file paths and/or directory paths
- **FR1.2**: Add `-d, --recurse_depth` parameter to control recursion depth (default: unlimited/recursive)
- **FR1.3**: These parameters must be available to any goal without requiring changes to `prompt.yaml`

#### FR2: File Discovery and Reading
- **FR2.1**: Support reading individual files specified in `--context`
- **FR2.2**: Support reading directories with configurable recursion depth
- **FR2.3**: Default behavior: recursively traverse directories
- **FR2.4**: With `--recurse_depth N`: limit traversal to N levels deep (0 = current directory only)
- **FR2.5**: Respect `.gitignore` patterns when traversing directories
- **FR2.6**: Skip binary files automatically (detect via file inspection or extension)

#### FR3: Filtering and Limits
- **FR3.1**: Enforce maximum file size limit (configurable in KB via `claw.yaml`)
- **FR3.2**: Enforce maximum number of files per directory (default: 50, configurable)
- **FR3.3**: Exclude directories matching configured patterns (default: `.git/`, `node_modules/`, `target/`, etc.)
- **FR3.4**: Exclude files matching configured extensions (default: `.exe`, `.bin`, `.so`, `.dylib`, etc.)
- **FR3.5**: Issue warnings when limits are reached

#### FR4: Error Handling
- **FR4.1**: Support three error handling modes (configurable in `claw.yaml`):
  - **Strict**: Fail the entire goal and report all errors immediately
  - **Flexible**: Collect all errors/warnings and prompt user for approval before proceeding
  - **Ignore**: Log warnings but continue processing valid files
- **FR4.2**: Handle common error scenarios:
  - File/directory does not exist
  - Permission denied
  - File exceeds size limit
  - Directory exceeds file count limit
  - Binary file encountered
  - UTF-8 decoding errors

#### FR5: Output Format
- **FR5.1**: Generate a markdown-formatted context section
- **FR5.2**: Include the following structure:
  - Summary section explaining the format and purpose
  - Directory structure visualization
  - Individual file contents with clear path headers
- **FR5.3**: Append this formatted content deterministically to the rendered prompt (not via template variables)
- **FR5.4**: Format must be similar to Repomix but adapted for user-provided files (not just codebase)

#### FR6: Integration with Existing System
- **FR6.1**: Integrate with current prompt rendering pipeline in `runner.rs`
- **FR6.2**: Ensure context files are processed after `context_scripts` execution
- **FR6.3**: Maintain existing functionality for goals without `--context` parameter
- **FR6.4**: Refactor existing goals to optionally leverage file context system

### Non-Functional Requirements

#### NFR1: Performance
- Minimize I/O operations by reading each file only once
- Use efficient directory traversal (consider using `walkdir` crate)
- Fail fast when limits are exceeded in strict mode
- Stream or chunk large directory scans to avoid memory spikes

#### NFR2: Usability
- Clear, actionable error messages
- Progress indicators for large directory scans (optional enhancement)
- Sensible defaults requiring minimal configuration
- Helpful warnings when approaching limits

#### NFR3: Maintainability
- Clean separation of concerns: file discovery, validation, formatting, and injection
- Well-documented configuration options
- Comprehensive error types for different failure scenarios

#### NFR4: Compatibility
- Backward compatible with existing goals
- Works across Linux, macOS, and Windows
- Respects platform-specific path conventions

### Dependencies and Prerequisites
- `walkdir` crate for efficient directory traversal
- `ignore` crate for `.gitignore` pattern matching
- Existing dependencies: `anyhow`, `serde`, `tera`

## 3. Architecture & Design

### High-Level Architecture

```
CLI Arguments (--context, --recurse_depth)
         ↓
   File Discovery Module
   (discover files/dirs, apply recursion limit)
         ↓
   Validation & Filtering Module
   (size limits, type checks, exclusions)
         ↓
   Error Handling Module
   (strict/flexible/ignore modes)
         ↓
   Formatting Module
   (generate markdown structure)
         ↓
   Prompt Injection Module
   (append to rendered prompt)
         ↓
   LLM Execution (existing flow)
```

### Key Components

#### 3.1 Configuration Extension (`src/config.rs`)

Add new fields to `ClawConfig`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ClawConfig {
    // Existing fields
    pub llm_command: String,
    pub prompt_arg_template: String,

    // New fields for context management
    #[serde(default = "default_max_file_size_kb")]
    pub max_file_size_kb: u64,

    #[serde(default = "default_max_files_per_directory")]
    pub max_files_per_directory: usize,

    #[serde(default = "default_error_handling_mode")]
    pub error_handling_mode: ErrorHandlingMode,

    #[serde(default = "default_excluded_directories")]
    pub excluded_directories: Vec<String>,

    #[serde(default = "default_excluded_extensions")]
    pub excluded_extensions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorHandlingMode {
    Strict,
    Flexible,
    Ignore,
}

fn default_max_file_size_kb() -> u64 { 1024 } // 1MB
fn default_max_files_per_directory() -> usize { 50 }
fn default_error_handling_mode() -> ErrorHandlingMode { ErrorHandlingMode::Flexible }
fn default_excluded_directories() -> Vec<String> {
    vec![".git", "node_modules", "target", ".venv", "__pycache__"].iter().map(|s| s.to_string()).collect()
}
fn default_excluded_extensions() -> Vec<String> {
    vec!["exe", "bin", "so", "dylib", "dll", "o", "a"].iter().map(|s| s.to_string()).collect()
}
```

#### 3.2 CLI Extension (`src/cli.rs`)

Add new arguments to the CLI structure:

```rust
// In the Run subcommand or common args
#[arg(short = 'c', long = "context", num_args = 0..)]
pub context: Vec<PathBuf>,

#[arg(short = 'd', long = "recurse_depth")]
pub recurse_depth: Option<usize>,
```

#### 3.3 Context Module (`src/context.rs` - NEW)

This new module handles all file context operations:

##### 3.3.1 Core Types

```rust
pub struct ContextConfig {
    pub paths: Vec<PathBuf>,
    pub recurse_depth: Option<usize>,
    pub max_file_size_kb: u64,
    pub max_files_per_directory: usize,
    pub error_handling_mode: ErrorHandlingMode,
    pub excluded_directories: Vec<String>,
    pub excluded_extensions: Vec<String>,
}

pub struct DiscoveredFile {
    pub path: PathBuf,
    pub size: u64,
    pub relative_path: PathBuf, // For clean display in output
}

pub enum ContextError {
    FileNotFound(PathBuf),
    PermissionDenied(PathBuf),
    FileTooLarge { path: PathBuf, size: u64, limit: u64 },
    TooManyFiles { directory: PathBuf, count: usize, limit: usize },
    BinaryFile(PathBuf),
    Utf8Error(PathBuf),
    IoError { path: PathBuf, error: std::io::Error },
}

pub struct ContextResult {
    pub files: Vec<FileContent>,
    pub errors: Vec<ContextError>,
    pub warnings: Vec<String>,
}

pub struct FileContent {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub content: String,
}
```

##### 3.3.2 File Discovery

```rust
pub fn discover_files(config: &ContextConfig) -> Result<Vec<DiscoveredFile>> {
    // For each path in config.paths:
    //   - If file: add directly
    //   - If directory: use walkdir with depth limit
    // Apply exclusion filters
    // Return discovered files
}
```

##### 3.3.3 Validation & Reading

```rust
pub fn validate_and_read_files(
    files: Vec<DiscoveredFile>,
    config: &ContextConfig,
) -> ContextResult {
    // For each file:
    //   - Check size limit
    //   - Check if binary
    //   - Read content
    //   - Handle UTF-8 errors
    // Accumulate errors/warnings
    // Return ContextResult
}
```

##### 3.3.4 Error Handling

```rust
pub fn handle_errors(
    result: &ContextResult,
    mode: &ErrorHandlingMode,
) -> Result<bool> {
    // Match on mode:
    //   - Strict: return Err if any errors
    //   - Flexible: prompt user, return Ok(continue) or Err(abort)
    //   - Ignore: log warnings, return Ok(true)
}
```

##### 3.3.5 Formatting

```rust
pub fn format_context(result: &ContextResult) -> String {
    // Generate markdown structure:
    //   - <file_summary> with purpose, format, guidelines
    //   - <directory_structure> with tree view
    //   - <files> with individual file contents
    // Return formatted string
}
```

#### 3.4 Runner Integration (`src/runner.rs`)

Modify the goal execution flow to integrate context:

```rust
pub fn run_goal_with_context(
    goal: &LoadedGoal,
    args: &HashMap<String, String>,
    context_paths: Vec<PathBuf>,
    recurse_depth: Option<usize>,
    config: &ClawConfig,
) -> Result<()> {
    // 1. Execute context scripts (existing)
    let script_outputs = execute_context_scripts(&goal.config.context_scripts)?;

    // 2. Render prompt with scripts + args (existing)
    let mut rendered_prompt = render_prompt(&goal, args, &script_outputs)?;

    // 3. Process file context if provided (NEW)
    if !context_paths.is_empty() {
        let context_config = ContextConfig {
            paths: context_paths,
            recurse_depth,
            max_file_size_kb: config.max_file_size_kb,
            max_files_per_directory: config.max_files_per_directory,
            error_handling_mode: config.error_handling_mode.clone(),
            excluded_directories: config.excluded_directories.clone(),
            excluded_extensions: config.excluded_extensions.clone(),
        };

        let files = discover_files(&context_config)?;
        let result = validate_and_read_files(files, &context_config);

        // Handle errors based on mode
        handle_errors(&result, &context_config.error_handling_mode)?;

        // Format and append to prompt
        let context_section = format_context(&result);
        rendered_prompt.push_str("\n\n");
        rendered_prompt.push_str(&context_section);
    }

    // 4. Run LLM with final prompt (existing)
    run_llm(config, &rendered_prompt)?;

    Ok(())
}
```

### Data Structures and Types

See section 3.3.1 for core types.

### Integration Points with Existing Code

1. **`src/cli.rs`**: Add new CLI arguments
2. **`src/config.rs`**: Extend `ClawConfig` with context settings
3. **`src/runner.rs`**:
   - Modify `run_goal` or create `run_goal_with_context`
   - Integrate context processing after script execution but before LLM invocation
4. **`src/main.rs`**: Pass new CLI arguments through to runner

### Output Format Specification

```markdown
# Context Files

This document contains user-provided files that are relevant to the current task.

## Purpose
This section includes files and directories specified by the user via the `--context` parameter.
The contents are formatted for easy consumption by AI systems for analysis, code review, or other tasks.

## Format
The content is organized as follows:
1. This summary section
2. Directory structure of included files
3. Individual file entries, each consisting of:
   - File path as a header
   - Full contents of the file in a code block

## Usage Guidelines
- These files provide additional context for the current goal
- File paths are relative to the working directory where claw was invoked
- Some files may have been excluded based on size limits, type restrictions, or configured exclusions

## Notes
- Maximum file size: {max_file_size_kb} KB
- Maximum files per directory: {max_files_per_directory}
- Excluded directories: {excluded_directories}
- Excluded extensions: {excluded_extensions}
- Recursion depth: {recurse_depth or "unlimited"}

---

## Directory Structure

```
{tree_view_of_included_files}
```

---

## Files

### {relative_path_1}

```
{file_content_1}
```

### {relative_path_2}

```
{file_content_2}
```

... (repeat for all files)
```

## 4. Implementation Plan

The implementation is broken down into modular, independent tasks that can be developed and tested in isolation.

### Phase 1: Foundation (Configuration & CLI)

**Task 1.1: Extend Configuration Schema**
- Add new fields to `ClawConfig` in `src/config.rs`
- Implement default functions for each new field
- Update example `assets/claw.yaml` with new configuration options
- **Dependencies**: None
- **Deliverable**: Extended `ClawConfig` struct with context management settings

**Task 1.2: Add CLI Parameters**
- Add `--context` and `--recurse_depth` arguments to CLI in `src/cli.rs`
- Ensure proper parsing and validation
- **Dependencies**: None
- **Deliverable**: New CLI parameters available

**Task 1.3: Update Documentation**
- Document new configuration options in example config
- Add comments explaining each setting
- **Dependencies**: Task 1.1
- **Deliverable**: Clear documentation for configuration

### Phase 2: Core Context Module

**Task 2.1: Create Context Module Skeleton**
- Create `src/context.rs` with module structure
- Define all types: `ContextConfig`, `DiscoveredFile`, `FileContent`, `ContextError`, `ContextResult`
- Add module to `src/main.rs` or `src/lib.rs`
- **Dependencies**: Task 1.1 (needs `ErrorHandlingMode` type)
- **Deliverable**: Well-typed context module foundation

**Task 2.2: Implement File Discovery**
- Implement `discover_files()` function
- Use `walkdir` crate for directory traversal
- Apply recursion depth limits
- Apply directory and extension exclusions
- **Dependencies**: Task 2.1
- **Deliverable**: Working file discovery with filtering

**Task 2.3: Implement Validation & Reading**
- Implement `validate_and_read_files()` function
- Check file sizes against limits
- Detect and skip binary files
- Read text files with UTF-8 validation
- Count files per directory and enforce limits
- **Dependencies**: Task 2.2
- **Deliverable**: Safe file reading with comprehensive validation

**Task 2.4: Implement Error Handling**
- Implement `handle_errors()` function
- Support all three modes: strict, flexible, ignore
- For flexible mode: implement user prompt for approval
- **Dependencies**: Task 2.3
- **Deliverable**: Configurable error handling

**Task 2.5: Implement Formatting**
- Implement `format_context()` function
- Generate markdown structure per specification
- Create directory tree visualization
- Format individual files with headers
- **Dependencies**: Task 2.3
- **Deliverable**: Clean markdown output formatter

### Phase 3: Integration

**Task 3.1: Integrate with Runner**
- Modify `src/runner.rs` to accept context parameters
- Call context module functions in appropriate order
- Append formatted context to rendered prompt
- **Dependencies**: Tasks 2.2, 2.3, 2.4, 2.5
- **Deliverable**: End-to-end context injection working

**Task 3.2: Wire CLI to Runner**
- Update `src/main.rs` to pass CLI arguments to runner
- Ensure proper error propagation
- **Dependencies**: Task 1.2, Task 3.1
- **Deliverable**: Full CLI-to-execution pipeline

**Task 3.3: Update Assets**
- Update `assets/claw.yaml` with new default configuration
- Ensure first-time setup includes context config
- **Dependencies**: Task 1.1, Task 1.3
- **Deliverable**: Proper defaults for new installations

### Phase 4: Testing & Refinement

**Task 4.1: Unit Tests**
- Write tests for file discovery with various exclusions
- Write tests for validation logic
- Write tests for error handling modes
- Write tests for formatting output
- **Dependencies**: Phase 2 complete
- **Deliverable**: Comprehensive unit test coverage

**Task 4.2: Integration Tests**
- Test end-to-end flow with real files and directories
- Test recursion depth limits
- Test size limit enforcement
- Test error modes
- **Dependencies**: Phase 3 complete
- **Deliverable**: Working integration tests

**Task 4.3: Edge Case Testing**
- Test with empty directories
- Test with permission-denied scenarios
- Test with symbolic links
- Test with very large directory trees
- Test with mixed binary/text files
- **Dependencies**: Task 4.2
- **Deliverable**: Robust edge case handling

### Phase 5: Goal Refactoring (Optional Enhancement)

**Task 5.1: Identify Refactor Candidates**
- Review existing goals in `assets/goals/`
- Identify goals that could benefit from file context
- **Dependencies**: Phase 3 complete
- **Deliverable**: List of goals to refactor

**Task 5.2: Refactor Existing Goals**
- Update goals to use `--context` instead of manual file reading
- Simplify `context_scripts` where applicable
- Update goal descriptions and documentation
- **Dependencies**: Task 5.1
- **Deliverable**: Modernized goals using new context system

### Parallelization Opportunities

The following tasks can be worked on in parallel:
- **Phase 1**: All tasks (1.1, 1.2, 1.3) are independent
- **Phase 2**: After 2.1, tasks 2.2 and 2.4 are independent
- **Phase 2**: After 2.3, tasks 2.4 and 2.5 are independent
- **Phase 4**: After Phase 3, tasks 4.1 and 4.2 can be done concurrently

### Task Dependencies Diagram

```
1.1 (Config) ──┬──> 2.1 (Module) ──> 2.2 (Discovery) ──> 2.3 (Validation) ──┬──> 2.4 (Error Handling) ──┐
1.2 (CLI) ─────┤                                                            └──> 2.5 (Formatting) ──────┤
1.3 (Docs) ────┘                                                                                         │
                                                                                                         ├──> 3.1 (Runner) ──┬──> 3.2 (Wire) ──> 4.2 (Integration Tests) ──> 4.3 (Edge Cases)
                                                                                                         │                   │
                                                                                                         └───────────────────┴──> 4.1 (Unit Tests)
                                                                                                                             │
                                                                                                                             └──> 5.1 ──> 5.2
```

## 5. Testing Strategy

### Unit Tests

#### Configuration Tests (`tests/config_tests.rs`)
- Test default value functions
- Test deserialization from YAML with various combinations
- Test `ErrorHandlingMode` enum parsing

#### Context Module Tests (`tests/context_tests.rs`)
- **File Discovery**:
  - Discover files in flat directory
  - Discover files with recursion
  - Respect recursion depth limit
  - Apply directory exclusions
  - Apply extension exclusions
  - Handle symlinks appropriately

- **Validation**:
  - Reject files exceeding size limit
  - Detect binary files correctly
  - Handle UTF-8 encoding errors
  - Count files per directory correctly
  - Enforce per-directory file limits

- **Error Handling**:
  - Strict mode fails on first error
  - Ignore mode continues despite errors
  - Flexible mode collects errors (mock user input)

- **Formatting**:
  - Generate correct markdown structure
  - Escape special characters if needed
  - Create accurate directory tree
  - Format file contents correctly

### Integration Tests

#### End-to-End Tests (`tests/integration_tests.rs`)
- Create temporary test directories with known structure
- Run claw with `--context` pointing to test directories
- Verify formatted output contains expected files
- Verify limits are enforced
- Verify error modes work correctly

#### CLI Tests
- Parse `--context` with multiple paths
- Parse `--recurse_depth` with valid/invalid values
- Handle missing or invalid paths gracefully

### Edge Cases

1. **Empty directory**: Should succeed with warning
2. **Permission denied**: Should handle per error mode
3. **Symbolic links**: Should follow or skip based on configuration
4. **Mixed binary/text**: Should skip binary, include text
5. **Deeply nested directories**: Should respect depth limit
6. **Files with special characters in names**: Should handle correctly
7. **Very large files**: Should reject per size limit
8. **Concurrent file access**: Should handle locked files gracefully
9. **Non-existent paths**: Should report clear error
10. **Relative vs absolute paths**: Should handle both correctly

### Acceptance Criteria

#### AC1: Basic File Inclusion
- Given: A goal and a single text file path via `--context`
- When: The goal is executed
- Then: The file contents appear in the formatted context section of the prompt

#### AC2: Directory Inclusion with Recursion
- Given: A goal and a directory path via `--context`
- When: The goal is executed with default settings
- Then: All text files in the directory and subdirectories are included

#### AC3: Recursion Depth Limit
- Given: A nested directory structure and `--recurse_depth 1`
- When: The goal is executed
- Then: Only files in the root and immediate subdirectories are included

#### AC4: Size Limit Enforcement
- Given: A file exceeding `max_file_size_kb`
- When: Error mode is strict
- Then: The goal fails with a clear error message

#### AC5: Directory File Count Limit
- Given: A directory with 60 files and limit set to 50
- When: Error mode is flexible
- Then: User is prompted about the excess and can choose to continue or abort

#### AC6: Exclusion Filters
- Given: A directory containing `.git/`, `node_modules/`, and `.exe` files
- When: Default exclusions are active
- Then: These files/directories are not included in the context

#### AC7: Error Mode - Ignore
- Given: Multiple files with various issues (too large, permission denied, binary)
- When: Error mode is ignore
- Then: Valid files are included, errors are logged, and execution continues

#### AC8: Markdown Format
- Given: Multiple files from different directories
- When: Context is formatted
- Then: Output matches the specified markdown structure with summary, tree, and file sections

#### AC9: Backward Compatibility
- Given: An existing goal without `--context` parameter
- When: The goal is executed normally
- Then: It works exactly as before without any context section

#### AC10: Goal Refactoring
- Given: An existing goal that manually reads files via `context_scripts`
- When: Refactored to use `--context`
- Then: The behavior is equivalent but with cleaner configuration

### Test Data Setup

Create a `tests/fixtures/` directory with:
- Sample text files of various sizes
- Binary files for detection testing
- Nested directory structures
- Files with special characters in names
- `.gitignore` file for exclusion testing
- Symlinks to test link handling

### Manual Testing Checklist

- [ ] Install claw with new feature
- [ ] Run goal with single file: `claw my-goal --context file.txt`
- [ ] Run goal with directory: `claw my-goal --context ./src/`
- [ ] Run goal with mixed: `claw my-goal --context file.txt ./docs/ README.md`
- [ ] Test recursion limit: `claw my-goal --context ./src/ --recurse_depth 2`
- [ ] Test with non-existent file (should error clearly)
- [ ] Test with very large file (should respect limit)
- [ ] Test with binary file (should skip)
- [ ] Verify markdown format is clean and readable
- [ ] Test each error mode: strict, flexible, ignore
- [ ] Verify configuration options work as expected
- [ ] Test on Linux, macOS, and Windows
- [ ] Verify existing goals still work
- [ ] Test performance with large directory (e.g., 1000+ files)

---

## Summary

This specification outlines a comprehensive file context system for claw that enables users to seamlessly include arbitrary files and directories as context for any goal. The design prioritizes safety through configurable limits, flexibility through error handling modes, and usability through a clean markdown format. The modular implementation plan allows for incremental development and testing, while maintaining full backward compatibility with existing goals.
