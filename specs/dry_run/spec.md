# Dry Run Feature Specification

## Overview

### Problem Statement and Goals
Currently, `claw` always starts the LLM application after a goal is rendered, providing no visibility into the actual prompt that will be sent. This makes it difficult for developers and users to:
- Debug and refine goal templates
- Verify that context scripts are producing expected output
- Review prompts before sending them to the LLM
- Document example prompts for goals
- Test prompt rendering without consuming LLM API credits

The dry-run feature addresses this by allowing users to render and view the complete prompt that would be sent to the LLM, with optional output to a file.

### Target Users and Use Cases
- **Developers**: Debugging goal templates, verifying template variable substitution, testing context script execution
- **Users**: Reviewing prompts before execution, understanding what context is being sent to the LLM, saving prompts for documentation

### Success Criteria
1. The dry-run output must match **exactly** what would be sent to the LLM in a normal execution
2. All goal features must work identically in dry-run mode (parameters, context scripts, file context)
3. Output can be directed to stdout or saved to a file
4. Error handling maintains parity with normal goal execution
5. No special formatting or headers - pure rendered prompt text only

## Requirements

### Functional Requirements

#### FR1: New Subcommand
- Implement a new `dry-run` subcommand with the following syntax:
  ```bash
  claw dry-run <goal> [OPTIONS] [-- <goal_params>]
  ```

#### FR2: Output Flag
- Support both short and long forms for output file specification:
  - Long form: `--output <file>`
  - Short form: `-o <file>`
- When output file is specified:
  - Write prompt to file only (not to stdout)
  - Silently overwrite existing files
  - Print confirmation message: `"Dry run output written to <file>"`

#### FR3: Goal Parameter Support
- Accept all the same parameters as normal goal execution:
  - `--context <paths>`: Include file/directory context
  - `--recurse_depth <n>`: Control directory recursion depth
  - `-- <goal_params>`: Pass template arguments after `--` separator
- Example:
  ```bash
  claw dry-run code-review --context src/ --recurse_depth 2 -- --scope auth --format json
  ```

#### FR4: Context Script Execution
- Execute all context scripts defined in the goal's `context_scripts` section
- Capture and include their output exactly as would happen in normal execution
- This ensures the dry-run shows the real, complete prompt

#### FR5: Prompt Rendering
- Render the complete prompt through the Tera templating engine
- Include all template variables:
  - `Args`: Command-line arguments passed via `-- <goal_params>`
  - `Context`: Output from executed context scripts
- Process file context (from `--context` flag) and append to prompt
- Output must be byte-for-byte identical to what would be sent to the LLM

#### FR6: Output Format
- **No special formatting or headers**
- Output the pure rendered prompt text exactly as the LLM would receive it
- No markers, no banners, no metadata
- When writing to file: use UTF-8 encoding

#### FR7: Scope Limitations
- Only works with goal execution (not with `claw pass` subcommand)
- Not applicable when no goal is specified (interactive mode)
- Not applicable with `claw add` or `claw list` commands

### Non-Functional Requirements

#### NFR1: Performance
- Dry-run execution should have negligible overhead compared to normal goal execution
- Context script execution performance remains identical to normal execution
- File I/O should not introduce significant delays

#### NFR2: Error Handling
- If `run_goal` logic fails, display the error and exit
- Provide detailed error messages for context script failures (include stderr output)
- When file write fails, display clear error message with file path and reason
- Maintain consistency with existing error handling patterns in codebase

#### NFR3: Exit Codes
Implement specific exit codes if complexity is manageable:
- `0`: Successful dry-run (prompt rendered and output successfully)
- `1`: General error (goal not found, parameter validation failed)
- `2`: Context script execution failed
- `3`: File write error (permission denied, disk full, etc.)

If specific exit codes introduce excessive complexity, fall back to:
- `0`: Success
- `1`: Any error

#### NFR4: Backward Compatibility
- No changes to existing command behavior
- Existing goals continue to work unchanged
- No modifications to `prompt.yaml` format required

### Dependencies and Prerequisites
- Rust standard library I/O modules (`std::fs`, `std::io`)
- Existing `run_goal` function and prompt rendering logic
- Existing parameter validation system
- Existing context management system

## Architecture & Design

### High-Level Architecture
The dry-run feature is a thin layer on top of the existing goal execution pipeline:

```
User Command
    ↓
CLI Parser (clap)
    ↓
DryRun Subcommand Handler
    ↓
Goal Loading & Validation ← (reuse existing)
    ↓
Parameter Validation ← (reuse existing)
    ↓
Context Script Execution ← (reuse existing)
    ↓
Prompt Rendering (Tera) ← (reuse existing)
    ↓
File Context Processing ← (reuse existing)
    ↓
Output Handler (NEW)
    ├─→ stdout (if no file specified)
    └─→ file (if --output provided)
```

### Key Components and Responsibilities

#### 1. CLI Extension (`src/cli.rs`)
**Responsibility**: Define the `DryRun` subcommand structure

```rust
#[derive(Subcommand, Debug)]
pub enum Subcommands {
    // ... existing subcommands ...

    /// Render a goal's prompt without executing the LLM
    DryRun {
        /// Name of the goal to render
        #[arg(required = true)]
        goal_name: String,

        /// Optional file path to write the rendered prompt
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,

        /// Files or directories to include as context
        #[arg(short = 'c', long = "context", num_args = 0..)]
        context: Vec<PathBuf>,

        /// Maximum recursion depth when scanning directories
        #[arg(short = 'd', long = "recurse_depth")]
        recurse_depth: Option<usize>,

        /// Arbitrary arguments for the prompt template
        #[arg(last = true)]
        template_args: Vec<String>,
    },
}
```

#### 2. Dry-Run Command Handler (`src/commands/dry_run.rs` - NEW)
**Responsibility**: Orchestrate dry-run execution and handle output

**Interface**:
```rust
pub fn handle_dry_run_command(
    goal_name: &str,
    output_file: Option<&PathBuf>,
    context_paths: &[PathBuf],
    recurse_depth: Option<usize>,
    template_args: &[String],
    claw_config: &ClawConfig,
) -> Result<()>
```

**Behavior**:
1. Call existing prompt rendering logic (extract from `run_goal`)
2. Receive the fully rendered prompt string
3. Route output based on `output_file` parameter:
   - If `None`: Write to stdout
   - If `Some(path)`: Write to file, print confirmation
4. Handle errors and return appropriate exit codes

#### 3. Prompt Rendering Extraction (`src/main.rs`)
**Responsibility**: Extract prompt rendering logic into reusable function

**Current situation**: `run_goal()` combines rendering and LLM execution.

**Required refactoring**:
```rust
// Extract this logic into a new function
fn render_goal_prompt(
    goal_name: &str,
    claw_config: &ClawConfig,
    template_args: &[String],
    context_paths: &[PathBuf],
    recurse_depth: Option<usize>,
) -> Result<String> {
    // ... all the existing rendering logic from run_goal ...
    // Returns the final rendered prompt string
}

// Update run_goal to use it
fn run_goal(...) -> Result<()> {
    let rendered_prompt = render_goal_prompt(...)?;
    runner::run_llm(claw_config, &rendered_prompt)?;
    Ok(())
}

// New handler uses it too
fn handle_dry_run_command(...) -> Result<()> {
    let rendered_prompt = render_goal_prompt(...)?;
    output_prompt(&rendered_prompt, output_file)?;
    Ok(())
}
```

#### 4. Output Handler (Part of `src/commands/dry_run.rs`)
**Responsibility**: Write prompt to stdout or file

```rust
fn output_prompt(prompt: &str, output_file: Option<&PathBuf>) -> Result<()> {
    match output_file {
        None => {
            // Write to stdout
            print!("{}", prompt);
            Ok(())
        }
        Some(path) => {
            // Write to file
            fs::write(path, prompt.as_bytes())
                .with_context(|| format!("Failed to write dry run output to {}", path.display()))?;

            // Print confirmation to stdout
            println!("Dry run output written to {}", path.display());
            Ok(())
        }
    }
}
```

### Data Structures and Types

#### Subcommand Definition
```rust
pub struct DryRunArgs {
    pub goal_name: String,
    pub output: Option<PathBuf>,
    pub context: Vec<PathBuf>,
    pub recurse_depth: Option<usize>,
    pub template_args: Vec<String>,
}
```

#### Exit Code Constants
```rust
// src/commands/dry_run.rs
const EXIT_SUCCESS: i32 = 0;
const EXIT_GENERAL_ERROR: i32 = 1;
const EXIT_SCRIPT_ERROR: i32 = 2;
const EXIT_FILE_WRITE_ERROR: i32 = 3;
```

### Integration Points with Existing Code

#### 1. CLI Parser (`src/cli.rs`)
- **Change Type**: Addition
- **Impact**: Add new `DryRun` variant to `Subcommands` enum
- **Risk**: Low (purely additive)

#### 2. Main Entry Point (`src/main.rs`)
- **Change Type**: Addition + Refactoring
- **Impact**:
  - Add new match arm for `Subcommands::DryRun`
  - Extract `render_goal_prompt` from `run_goal`
- **Risk**: Medium (refactoring existing working code)

#### 3. Commands Module (`src/commands/mod.rs`)
- **Change Type**: Addition
- **Impact**: Add `pub mod dry_run;`
- **Risk**: Low

#### 4. Runner Module (`src/runner.rs`)
- **Change Type**: None required
- **Impact**: None (existing `run_llm` function not called in dry-run mode)
- **Risk**: None

#### 5. Config Module (`src/config.rs`)
- **Change Type**: None required
- **Impact**: None (all existing functions reused as-is)
- **Risk**: None

## Implementation Plan

### Task Breakdown

#### Task 1: Create Command Handler Module
**Description**: Create the new `src/commands/dry_run.rs` file with output handling logic.

**Dependencies**: None

**Deliverables**:
- `src/commands/dry_run.rs` with:
  - `handle_dry_run_command()` function signature
  - `output_prompt()` helper function
  - Exit code constants
  - Basic error handling structure

**Parallelizable**: Yes (can be done independently)

---

#### Task 2: Extend CLI Parser
**Description**: Add the `DryRun` subcommand definition to `src/cli.rs`.

**Dependencies**: None

**Deliverables**:
- Updated `Subcommands` enum with `DryRun` variant
- All required arguments with short/long forms
- Proper clap annotations

**Parallelizable**: Yes (can be done in parallel with Task 1)

---

#### Task 3: Refactor Prompt Rendering Logic
**Description**: Extract prompt rendering from `run_goal()` into a reusable `render_goal_prompt()` function.

**Dependencies**: None (but should complete before Task 4)

**Deliverables**:
- New `render_goal_prompt()` function in `src/main.rs`
- Updated `run_goal()` to use extracted function
- Ensure all existing functionality preserved (parameters, context scripts, file context)

**Parallelizable**: No (blocking for Task 4)

---

#### Task 4: Connect Command Handler to Main
**Description**: Wire up the dry-run subcommand in the main entry point.

**Dependencies**: Task 1, Task 2, Task 3

**Deliverables**:
- New match arm in `src/main.rs` for `Subcommands::DryRun`
- Call to `handle_dry_run_command()` with proper argument mapping
- Call to `render_goal_prompt()` within the handler

**Parallelizable**: No (requires Tasks 1-3 complete)

---

#### Task 5: Implement Error Handling
**Description**: Add comprehensive error handling and exit codes.

**Dependencies**: Task 4

**Deliverables**:
- Specific exit codes for different error types
- Detailed error messages for:
  - Goal not found
  - Context script failures
  - File write errors
  - Parameter validation failures
- Error context using `anyhow::Context`

**Parallelizable**: No (requires Task 4)

---

#### Task 6: Write Unit Tests
**Description**: Create unit tests for dry-run functionality.

**Dependencies**: Task 1 (for output_prompt tests), can parallelize with Task 4-5 for integration tests

**Deliverables**:
- Tests for `output_prompt()`:
  - Test stdout output
  - Test file output
  - Test file overwrite behavior
  - Test file write error handling
- Tests for exit code logic
- Tests can live in `src/commands/dry_run.rs` test module

**Parallelizable**: Partially (output_prompt tests can be written early)

---

#### Task 7: Write Integration Tests
**Description**: Create end-to-end integration tests.

**Dependencies**: Task 4, Task 5

**Deliverables**:
- Integration test file (e.g., `tests/dry_run_integration.rs`)
- Test scenarios:
  - Dry-run with simple goal (no parameters, no context)
  - Dry-run with goal parameters
  - Dry-run with `--context` flag
  - Dry-run with context scripts
  - Dry-run with output file
  - Dry-run with all features combined
  - Error cases: nonexistent goal, invalid parameters
  - Verify output matches normal execution (except no LLM call)

**Parallelizable**: No (requires working implementation)

---

#### Task 8: Update Documentation
**Description**: Update README and add examples.

**Dependencies**: Task 7 (ideally test first, then document proven behavior)

**Deliverables**:
- Update `README.md` with dry-run examples
- Add dry-run section to usage guide
- Document exit codes
- Add troubleshooting tips

**Parallelizable**: No (requires working implementation)

### Task Dependencies Graph

```
Task 1 ────────┐
               ├───→ Task 4 ───→ Task 5 ───→ Task 7 ───→ Task 8
Task 2 ────────┤                              ↑
               │                              │
Task 3 ────────┘                              │
                                              │
Task 6 ────────────────────────────────────────┘
```

### Parallel Execution Opportunities

**Phase 1** (Parallel):
- Task 1: Create command handler module
- Task 2: Extend CLI parser
- Task 3: Refactor prompt rendering
- Task 6: Write unit tests for output_prompt

**Phase 2** (Sequential):
- Task 4: Connect handler to main

**Phase 3** (Sequential):
- Task 5: Implement error handling

**Phase 4** (Parallel):
- Task 6: Complete remaining unit tests
- Task 7: Write integration tests

**Phase 5** (Sequential):
- Task 8: Update documentation

## Testing Strategy

### Test Scenarios

#### Unit Tests

**Module**: `src/commands/dry_run.rs`

1. **Test: output_prompt_to_stdout**
   - Input: Sample prompt string, `output_file = None`
   - Expected: Prompt written to stdout
   - Verification: Capture stdout and verify content

2. **Test: output_prompt_to_file**
   - Input: Sample prompt string, `output_file = Some(temp_path)`
   - Expected: Prompt written to file, confirmation message on stdout
   - Verification: Read file and verify content matches exactly

3. **Test: output_prompt_overwrites_existing_file**
   - Setup: Create file with existing content
   - Input: New prompt string, `output_file` pointing to existing file
   - Expected: File overwritten with new content
   - Verification: Read file and verify only new content present

4. **Test: output_prompt_handles_write_error**
   - Setup: Create read-only file or directory
   - Input: Prompt string, `output_file` pointing to read-only path
   - Expected: Returns error with context
   - Verification: Check error message contains file path and reason

5. **Test: exit_code_on_success**
   - Input: Successful dry-run
   - Expected: Exit code 0
   - Verification: Check return value

6. **Test: exit_code_on_script_error**
   - Input: Goal with failing context script
   - Expected: Exit code 2
   - Verification: Check error type and exit code

7. **Test: exit_code_on_file_write_error**
   - Input: File write failure
   - Expected: Exit code 3
   - Verification: Check error type and exit code

#### Integration Tests

**Module**: `tests/dry_run_integration.rs`

1. **Test: dry_run_simple_goal**
   - Setup: Create test goal with basic prompt (no parameters, no context)
   - Command: `claw dry-run test-goal`
   - Expected: Prompt output to stdout
   - Verification: Output matches expected rendered prompt

2. **Test: dry_run_with_parameters**
   - Setup: Create test goal that uses `{{ Args.param1 }}` in template
   - Command: `claw dry-run test-goal -- --param1=value1`
   - Expected: Prompt with substituted parameter value
   - Verification: Output contains "value1"

3. **Test: dry_run_with_context_scripts**
   - Setup: Create test goal with context script (e.g., `echo "script output"`)
   - Command: `claw dry-run test-goal`
   - Expected: Prompt includes context script output
   - Verification: Output contains "script output"

4. **Test: dry_run_with_file_context**
   - Setup: Create test goal and sample files
   - Command: `claw dry-run test-goal --context test_file.txt`
   - Expected: Prompt includes file context section
   - Verification: Output contains file contents

5. **Test: dry_run_to_file**
   - Setup: Create test goal
   - Command: `claw dry-run test-goal --output /tmp/dry_run_output.txt`
   - Expected:
     - File created with prompt content
     - Confirmation message on stdout
     - No prompt on stdout
   - Verification: Read file and verify content

6. **Test: dry_run_matches_normal_execution**
   - Setup: Create test goal with all features (parameters, context scripts, file context)
   - Method:
     - Run dry-run and capture output
     - Mock `run_llm` to capture what would be sent to LLM in normal execution
     - Compare both outputs
   - Expected: Byte-for-byte identical output
   - Verification: String equality check

7. **Test: dry_run_nonexistent_goal**
   - Command: `claw dry-run nonexistent-goal`
   - Expected: Error message "Goal 'nonexistent-goal' not found"
   - Verification: Check exit code 1 and error message

8. **Test: dry_run_invalid_parameters**
   - Setup: Create test goal with required parameter
   - Command: `claw dry-run test-goal` (without required parameter)
   - Expected: Parameter validation error
   - Verification: Check exit code 1 and error message

9. **Test: dry_run_failing_context_script**
   - Setup: Create test goal with context script that exits non-zero
   - Command: `claw dry-run test-goal`
   - Expected: Error message with script details
   - Verification: Check exit code 2 and error contains script name/stderr

10. **Test: dry_run_all_features_combined**
    - Setup: Create complex test goal
    - Command: `claw dry-run test-goal --context src/ --recurse_depth 2 --output out.txt -- --param1=val1 --param2=val2`
    - Expected: All features work together correctly
    - Verification: File contains complete prompt with all elements

### Edge Cases

1. **Empty goal prompt**: Goal with empty prompt template
2. **Very large prompt**: Goal that generates >1MB prompt (stress test)
3. **Unicode content**: Goal with non-ASCII characters in template and context
4. **Special characters in file paths**: Output file with spaces, quotes, etc.
5. **Circular template references**: Tera template with circular includes (should error)
6. **Missing template variables**: Template uses undefined `{{ Args.missing }}` (Tera should error)
7. **Binary files in context**: `--context` points to binary file (should skip like normal)

### Acceptance Criteria

✅ **AC1**: Running `claw dry-run <goal>` outputs the exact prompt that would be sent to the LLM

✅ **AC2**: Output to file works correctly with `--output` flag (both short and long forms)

✅ **AC3**: All goal parameters (`--context`, `--recurse_depth`, template args) work identically to normal execution

✅ **AC4**: Context scripts execute and their output is included in the rendered prompt

✅ **AC5**: File output is silent to stdout except for confirmation message

✅ **AC6**: Existing files are overwritten without prompt

✅ **AC7**: Errors are clearly reported with appropriate exit codes

✅ **AC8**: Unit test coverage >80% for new code

✅ **AC9**: All integration test scenarios pass

✅ **AC10**: Documentation is updated with examples and usage guide

### Test Execution Plan

1. **During Development**: Run unit tests continuously (`cargo test --lib`)
2. **Before PR**: Run full test suite (`cargo test`)
3. **CI Pipeline**: Automated test execution on all commits
4. **Manual Verification**: Test all examples from documentation actually work

---

## Appendix

### Example Usage

#### Basic dry-run
```bash
$ claw dry-run code-review
You are an expert code reviewer. Please review the following code...
[full rendered prompt]
```

#### With parameters
```bash
$ claw dry-run generate-tests -- --language=rust --framework=pytest
```

#### With file context
```bash
$ claw dry-run analyze-code --context src/main.rs src/config.rs
```

#### Save to file
```bash
$ claw dry-run code-review --output review_prompt.txt
Dry run output written to review_prompt.txt
```

#### Complex example
```bash
$ claw dry-run refactor \
    --context src/ \
    --recurse_depth 3 \
    --output prompts/refactor_$(date +%Y%m%d).txt \
    -- --scope=authentication --style=functional
Dry run output written to prompts/refactor_20251008.txt
```

### Error Message Examples

#### Goal not found
```
Error: Goal 'unknown-goal' not found in local or global configuration
```

#### Context script failed
```
Error: Context script 'git_diff' failed with exit code 128

Script command: git diff --staged
Error output: fatal: not a git repository (or any of the parent directories): .git
```

#### File write error
```
Error: Failed to write dry run output to /readonly/path/output.txt
Caused by: Permission denied (os error 13)
```

#### Parameter validation error
```
Error: Missing required parameter 'language' for goal 'generate-tests'
Run 'claw generate-tests --explain' to see parameter details
```
