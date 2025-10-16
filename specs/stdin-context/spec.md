# Stdin Context Passing & Prompt Receiver Architecture Specification

## Overview

### Problem Statement
Currently, claw passes rendered prompts (including context) to LLM CLI tools via command-line arguments using template substitution with `{{prompt}}`. This approach has two critical limitations:

1. **Shell length limits**: Command-line arguments are limited by the shell's maximum argument size (typically ARG_MAX), which prevents passing large context files
2. **Rigid architecture**: The current implementation is tightly coupled to process-based CLI execution, making it difficult to extend to other integration points like IDEs or APIs

### Goals
1. Eliminate shell command-line length limitations by supporting stdin-based prompt passing
2. Create a flexible `PromptReceiver` abstraction that enables multiple integration strategies
3. Maintain backward compatibility with existing configurations
4. Provide clear migration path for users with large contexts

### Target Users
- Existing claw users who use the `--context` parameter with large codebases
- Future users who want to integrate claw with IDEs (starting with VSCode)
- Developers extending claw with new LLM tool integrations

### Success Criteria
1. Users can pass arbitrarily large contexts without hitting shell limits
2. Existing configs continue to work without modification
3. New receiver architecture is extensible for future IDE integrations
4. Unit tests verify prompt passing logic
5. Manual testing with `cat` command confirms stdin functionality

---

## Requirements

### Functional Requirements

#### FR-1: Prompt Receiver Abstraction
Create a `PromptReceiver` trait/interface that:
- Defines a contract for sending rendered prompts
- Accepts a rendered prompt string
- Returns success/failure status
- Enables implementations to use whatever delivery mechanism they need (CLI args, stdin, IPC, API, etc.)

#### FR-2: Generic Receiver Implementation
Implement a `Generic` receiver that:
- Uses `llm_command` from claw configuration
- Supports flexible prompt passing based on `prompt_arg_template`:
  - If `{{prompt}}` placeholder exists → pass as command-line argument (current behavior)
  - If `{{prompt}}` placeholder NOT present → pipe prompt to stdin
- Handles stdin write failures with clear error messages

#### FR-3: ClaudeCli Receiver Implementation
Implement a `ClaudeCli` receiver that:
- Hardcodes `llm_command = "claude"`
- Ignores the `llm_command` config field
- Respects `prompt_arg_template` with same stdin/{{prompt}} logic as Generic
- Serves as a convenience receiver for Claude CLI users

#### FR-4: Configuration Support
Add `receiver_type` configuration option:
```toml
receiver_type = "Generic"  # or "ClaudeCli"
llm_command = "my-llm"
prompt_arg_template = "--message {{prompt}}"  # or "-p extra_flags" for stdin
```

#### FR-5: Backward Compatibility
- Existing configs without `receiver_type` default to `Generic`
- Existing `{{prompt}}` templates continue to work unchanged
- No breaking changes to command-line interface

#### FR-6: Migration Warning
When using `{{prompt}}` substitution:
- If prompt size exceeds 1MB → show migration warning
- Warning message: "Note: Your prompt is over 1MB. Consider removing {{prompt}} from prompt_arg_template to use stdin for better handling of large contexts."

#### FR-7: Error Handling
When stdin piping fails:
- Display clear error message: "Failed to pass prompt to LLM via stdin. Check if your LLM command supports stdin input, or try using {{prompt}} in prompt_arg_template."
- Include underlying IO error details
- Exit with non-zero status

#### FR-8: Unchanged Behavior
- `dry-run` command: continues to output rendered prompt to stdout
- `pass` command: remains unaffected (no prompt involved)

### Non-Functional Requirements

#### NFR-1: Performance
- No performance degradation for small prompts
- Efficient buffered writing for large prompts to stdin
- No artificial timeouts on stdin writes

#### NFR-2: Cross-Platform Compatibility
- Use Rust best practices for cross-platform stdin handling
- Support Windows, macOS, and Linux equally

#### NFR-3: Maintainability
- Clear separation between receiver interface and implementations
- Each receiver type is independently testable
- Well-documented trait methods and implementation requirements

#### NFR-4: Extensibility
- Architecture supports future receiver types without modifying core logic
- Easy to add new receiver variants to the enum
- Clear patterns for receiver configuration

### Dependencies and Prerequisites
- Rust standard library for process spawning and stdin handling
- Existing claw configuration system
- No new external dependencies required

---

## Architecture & Design

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      claw main.rs                        │
│  ┌───────────────────────────────────────────────────┐  │
│  │  render_goal_prompt() → rendered_prompt           │  │
│  └───────────────────────────────────────────────────┘  │
│                          ↓                               │
│  ┌───────────────────────────────────────────────────┐  │
│  │  run_goal() → receiver.send_prompt(prompt)        │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│              PromptReceiver Trait (runner.rs)            │
│  ┌───────────────────────────────────────────────────┐  │
│  │  fn send_prompt(&self, prompt: &str) -> Result   │  │
│  │                                                   │  │
│  │  Provides interface for implementations to       │  │
│  │  deliver prompts using any mechanism they need   │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
          ↓                                    ↓
┌────────────────────────┐      ┌──────────────────────────┐
│  GenericReceiver       │      │  ClaudeCliReceiver       │
│  ┌──────────────────┐  │      │  ┌────────────────────┐  │
│  │ uses config:     │  │      │  │ hardcoded:         │  │
│  │  - llm_command   │  │      │  │  - "claude"        │  │
│  │  - prompt_arg_   │  │      │  │ uses config:       │  │
│  │    template      │  │      │  │  - prompt_arg_     │  │
│  │                  │  │      │  │    template        │  │
│  │ Implements:      │  │      │  │                    │  │
│  │  - stdin piping  │  │      │  │ Implements:        │  │
│  │  - arg passing   │  │      │  │  - stdin piping    │  │
│  └──────────────────┘  │      │  │  - arg passing     │  │
└────────────────────────┘      │  └────────────────────┘  │
                                └──────────────────────────┘
```

### Key Components

#### 1. PromptReceiver Trait
**Location**: `src/runner.rs`

**Responsibilities**:
- Define the contract for sending prompts to different targets
- Provide a consistent interface that abstracts implementation details
- Enable implementations to use whatever delivery mechanism suits their needs

**Interface**:
```rust
pub trait PromptReceiver {
    /// Sends a rendered prompt to the target system.
    ///
    /// Implementations are responsible for:
    /// - Choosing the appropriate delivery mechanism (stdin, args, IPC, API, etc.)
    /// - Handling all communication details
    /// - Reporting errors clearly
    ///
    /// Returns Ok(()) on success, Err on failure.
    fn send_prompt(&self, prompt: &str) -> Result<()>;

    /// Returns a human-readable name for this receiver type.
    fn name(&self) -> &str;
}
```

#### 2. ReceiverType Enum
**Location**: `src/config.rs`

**Responsibilities**:
- Enumerate all supported receiver types
- Support serialization/deserialization for config
- Provide factory method for creating receivers

**Definition**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReceiverType {
    Generic,
    ClaudeCli,
    // Future: VSCode, CursorIDE, API, etc.
}

impl Default for ReceiverType {
    fn default() -> Self {
        ReceiverType::Generic
    }
}
```

#### 3. GenericReceiver
**Location**: `src/runner.rs`

**Responsibilities**:
- Execute arbitrary CLI commands from config
- Implement stdin/{{prompt}} detection logic
- Handle argument template parsing and substitution
- Manage child process creation and stdin piping

**Key Logic**:
```rust
pub struct GenericReceiver {
    llm_command: String,
    prompt_arg_template: String,
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
}
```

**Stdin Implementation**:
- Use `std::process::Command` with `.stdin(Stdio::piped())`
- Spawn child process
- Write prompt to stdin using buffered writer
- Close stdin and wait for child to complete
- Handle broken pipe and other IO errors

**Argument Implementation**:
- Parse `prompt_arg_template` with `shlex::split`
- Substitute `{{prompt}}` with actual prompt
- Spawn child process with arguments
- No stdin interaction

#### 4. ClaudeCliReceiver
**Location**: `src/runner.rs`

**Responsibilities**:
- Convenience receiver for Claude CLI
- Hardcode "claude" as command
- Otherwise identical to GenericReceiver

**Key Logic**:
```rust
pub struct ClaudeCliReceiver {
    prompt_arg_template: String,
}

impl PromptReceiver for ClaudeCliReceiver {
    fn send_prompt(&self, prompt: &str) -> Result<()> {
        // Same logic as GenericReceiver but with hardcoded "claude"
        let generic = GenericReceiver {
            llm_command: "claude".to_string(),
            prompt_arg_template: self.prompt_arg_template.clone(),
        };
        generic.send_prompt(prompt)
    }
}
```

#### 5. Configuration Integration
**Location**: `src/config.rs`

Add to `ClawConfig`:
```rust
pub struct ClawConfig {
    pub llm_command: String,
    pub prompt_arg_template: String,
    pub receiver_type: Option<ReceiverType>,  // New field
    // ... existing fields
}
```

**Factory Method**:
```rust
impl ClawConfig {
    pub fn create_receiver(&self) -> Box<dyn PromptReceiver> {
        let receiver_type = self.receiver_type.clone()
            .unwrap_or(ReceiverType::Generic);

        match receiver_type {
            ReceiverType::Generic => {
                Box::new(GenericReceiver {
                    llm_command: self.llm_command.clone(),
                    prompt_arg_template: self.prompt_arg_template.clone(),
                })
            }
            ReceiverType::ClaudeCli => {
                Box::new(ClaudeCliReceiver {
                    prompt_arg_template: self.prompt_arg_template.clone(),
                })
            }
        }
    }
}
```

### Data Structures and Types

#### Prompt Size Calculation
```rust
fn check_prompt_size_warning(prompt: &str, template: &str) {
    const MB: usize = 1024 * 1024;
    if template.contains("{{prompt}}") && prompt.len() > MB {
        eprintln!("⚠️  Warning: Your prompt is over 1MB. Consider removing {{{{prompt}}}} from prompt_arg_template to use stdin for better handling of large contexts.");
    }
}
```

#### Error Types
```rust
#[derive(Debug)]
pub enum PromptReceiverError {
    CommandNotFound(String),
    StdinWriteFailed {
        command: String,
        error: std::io::Error,
    },
    ProcessFailed {
        command: String,
        status: std::process::ExitStatus,
    },
}
```

### Integration Points

#### main.rs Changes
**Current**:
```rust
fn run_goal(...) -> Result<()> {
    let rendered_prompt = render_goal_prompt(...)?;
    runner::run_llm(claw_config, &rendered_prompt)?;
    Ok(())
}
```

**New**:
```rust
fn run_goal(...) -> Result<()> {
    let rendered_prompt = render_goal_prompt(...)?;

    // Check size warning
    check_prompt_size_warning(&rendered_prompt, &claw_config.prompt_arg_template);

    // Create receiver and send prompt
    let receiver = claw_config.create_receiver();
    receiver.send_prompt(&rendered_prompt)?;

    Ok(())
}
```

#### runner.rs Refactoring
**Remove**: Current `run_llm` function

**Add**:
- `PromptReceiver` trait
- `GenericReceiver` struct + impl
- `ClaudeCliReceiver` struct + impl
- Helper functions for stdin/argument handling

**Keep unchanged**:
- `run_pass_through` function
- `execute_context_scripts` function

---

## Implementation Plan

### Phase 1: Core Architecture (Foundation)
**Tasks**:
1. Define `PromptReceiver` trait in `src/runner.rs`
2. Add `ReceiverType` enum to `src/config.rs`
3. Update `ClawConfig` struct with `receiver_type` field
4. Add factory method `create_receiver()` to `ClawConfig`
5. Write unit tests for configuration parsing with new field

**Dependencies**: None

**Deliverables**:
- Trait definition
- Config structure ready
- Tests passing

### Phase 2: Generic Receiver Implementation
**Tasks**:
1. Create `GenericReceiver` struct
2. Implement detection logic for `{{prompt}}` in template
3. Implement `send_via_argument()` method (refactor existing `run_llm` code)
4. Implement `send_via_stdin()` method using Rust best practices
5. Implement error handling with helpful messages
6. Write unit tests for detection logic
7. Write unit tests for argument parsing

**Dependencies**: Phase 1

**Deliverables**:
- Working `GenericReceiver`
- Both stdin and argument modes functional
- Unit tests passing

### Phase 3: ClaudeCli Receiver Implementation
**Tasks**:
1. Create `ClaudeCliReceiver` struct
2. Implement by delegating to `GenericReceiver` with hardcoded "claude"
3. Write unit tests for ClaudeCli-specific behavior

**Dependencies**: Phase 2

**Deliverables**:
- Working `ClaudeCliReceiver`
- Unit tests passing

### Phase 4: Integration & Refactoring
**Tasks**:
1. Update `run_goal()` in `src/main.rs` to use receiver abstraction
2. Add prompt size check and migration warning
3. Remove old `run_llm()` function from `src/runner.rs`
4. Ensure `dry-run` command still works correctly
5. Ensure `pass` command still works correctly
6. Update error messages throughout

**Dependencies**: Phase 2, Phase 3

**Deliverables**:
- Fully integrated system
- All commands working
- Old code removed

### Phase 5: Testing & Documentation
**Tasks**:
1. Manual testing with `cat` command as mock LLM:
   - Test with `{{prompt}}` in template
   - Test without `{{prompt}}` (stdin mode)
   - Test with large context (> 1MB) to verify no shell limits
   - Test with small context
2. Manual testing with actual `claude` CLI if available
3. Update README.md with `receiver_type` configuration option
4. Add example configs to README for both modes
5. Run full test suite

**Dependencies**: Phase 4

**Deliverables**:
- Manual test results documented
- README updated
- All tests passing
- Feature complete

### Task Sequencing
- **Sequential**: Phases must be completed in order (1 → 2 → 3 → 4 → 5)
- **Parallelizable within phases**: Unit tests can be written alongside implementation
- **Phase 2 & 3**: Could be partially parallelized, but Phase 2 should be mostly complete first

---

## Testing Strategy

### Unit Tests

#### Test 1: Prompt Detection Logic
**File**: `src/runner.rs`

**Test Cases**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_detects_prompt_placeholder() {
        let template = "--message {{prompt}}";
        assert!(template.contains("{{prompt}}"));
    }

    #[test]
    fn test_no_prompt_placeholder() {
        let template = "-p extra_flags";
        assert!(!template.contains("{{prompt}}"));
    }

    #[test]
    fn test_empty_template() {
        let template = "";
        assert!(!template.contains("{{prompt}}"));
    }
}
```

#### Test 2: ReceiverType Configuration
**File**: `src/config.rs`

**Test Cases**:
```rust
#[test]
fn test_receiver_type_defaults_to_generic() {
    let config = ClawConfig::default();
    assert_eq!(config.receiver_type, None);
    // Factory should create Generic
}

#[test]
fn test_receiver_type_claude_cli() {
    let toml = r#"
        receiver_type = "ClaudeCli"
        llm_command = "ignored"
    "#;
    let config: ClawConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.receiver_type, Some(ReceiverType::ClaudeCli));
}
```

#### Test 3: Prompt Size Warning
**File**: `src/runner.rs`

**Test Cases**:
```rust
#[test]
fn test_large_prompt_with_placeholder_shows_warning() {
    let prompt = "a".repeat(2 * 1024 * 1024); // 2MB
    let template = "--message {{prompt}}";
    // Capture stderr and verify warning appears
}

#[test]
fn test_large_prompt_without_placeholder_no_warning() {
    let prompt = "a".repeat(2 * 1024 * 1024); // 2MB
    let template = "-p flags";
    // Verify no warning
}
```

### Manual Testing

#### Test Scenario 1: Stdin Mode with `cat`
**Setup**:
```toml
# ~/.config/claw/config.toml
receiver_type = "Generic"
llm_command = "cat"
prompt_arg_template = ""
```

**Test**:
```bash
claw my-goal --context src/
```

**Expected**: Prompt is piped to `cat`, which outputs it to stdout

#### Test Scenario 2: Argument Mode with `cat`
**Setup**:
```toml
receiver_type = "Generic"
llm_command = "cat"
prompt_arg_template = "{{prompt}}"
```

**Test**:
```bash
claw my-goal --context src/
```

**Expected**: Prompt is passed as argument to `cat`

#### Test Scenario 3: Large Context (Stdin Mode)
**Setup**: Same as Scenario 1

**Test**:
```bash
claw my-goal --context very_large_codebase/ --recurse-depth 10
```

**Expected**: Works without shell limit errors, no warning shown

#### Test Scenario 4: Large Context (Argument Mode)
**Setup**: Same as Scenario 2

**Test**:
```bash
claw my-goal --context very_large_codebase/ --recurse-depth 10
```

**Expected**:
- May fail with shell limit errors (documenting the problem)
- Should show migration warning if prompt > 1MB

#### Test Scenario 5: ClaudeCli Receiver
**Setup**:
```toml
receiver_type = "ClaudeCli"
llm_command = "should-be-ignored"
prompt_arg_template = ""
```

**Test**:
```bash
claw my-goal
```

**Expected**: Executes `claude` (not "should-be-ignored"), uses stdin

### Acceptance Criteria
- ✅ All unit tests pass
- ✅ Manual testing confirms stdin piping works
- ✅ Manual testing confirms argument mode still works
- ✅ Large contexts don't hit shell limits in stdin mode
- ✅ Migration warning appears for large prompts with {{prompt}}
- ✅ Error messages are clear and helpful
- ✅ ClaudeCli receiver ignores llm_command config
- ✅ Backward compatibility maintained
- ✅ Dry-run and pass commands unaffected

---

## Configuration Examples

### Example 1: Generic Receiver with Stdin (Recommended for large contexts)
```toml
# ~/.config/claw/config.toml
receiver_type = "Generic"
llm_command = "claude"
prompt_arg_template = "--profile work"

# Other config...
max_file_size_kb = 1024
max_files_per_directory = 50
```

**Behavior**: Runs `claude --profile work` with prompt piped to stdin

### Example 2: Generic Receiver with Argument Mode
```toml
receiver_type = "Generic"
llm_command = "my-llm-cli"
prompt_arg_template = "chat --message {{prompt}}"
```

**Behavior**: Runs `my-llm-cli chat --message '<full-prompt>'`

### Example 3: ClaudeCli Receiver
```toml
receiver_type = "ClaudeCli"
prompt_arg_template = "--profile personal"
# llm_command is ignored for ClaudeCli
```

**Behavior**: Runs `claude --profile personal` with prompt via stdin

### Example 4: Backward Compatible (No receiver_type)
```toml
# Old config without receiver_type
llm_command = "aider"
prompt_arg_template = "--message {{prompt}}"
```

**Behavior**: Defaults to Generic receiver, uses argument mode

---

## Future Extensions

### Planned Receiver Types

#### VSCode Integration (`ReceiverType::VSCode`)
**Purpose**: Send prompts directly to VSCode's AI assistant panel

**Implementation Approach**:
- Use VSCode extension API or IPC mechanism
- Ignore `llm_command` and `prompt_arg_template`
- Require VSCode extension installed
- Configuration might include workspace path, profile settings

**Benefits**:
- No CLI tool needed
- Integrated IDE experience
- Direct access to VSCode context

### Potential Future Receivers
- `CursorIDE`: Integration with Cursor editor
- `API`: Direct HTTP API calls (OpenAI, Anthropic, etc.)
- `SSH`: Remote execution of prompts
- `Docker`: Execute LLM inside container

### Extensibility Considerations
The `PromptReceiver` trait architecture supports future receivers through:
1. Adding new enum variants to `ReceiverType`
2. Implementing `PromptReceiver` trait with whatever delivery mechanism is needed
3. Adding creation logic to factory method
4. No changes needed to core prompt rendering logic

Each new receiver implementation has complete freedom to implement prompt delivery in whatever way makes sense for that target system.

---

## Migration Guide

### For Users with Existing Configs

#### Current Config Works As-Is
Your existing configuration will continue to work:
```toml
llm_command = "claude"
prompt_arg_template = "--message {{prompt}}"
```

This automatically uses the `Generic` receiver in argument mode.

#### To Use Stdin Mode (Recommended)
Remove `{{prompt}}` from your template:
```toml
receiver_type = "Generic"  # Optional: this is the default
llm_command = "claude"
prompt_arg_template = "--profile work"
```

#### To Use ClaudeCli Receiver
```toml
receiver_type = "ClaudeCli"
prompt_arg_template = ""  # or any claude-specific flags
```

### No Breaking Changes
- All existing commands work unchanged
- Configuration is backward compatible
- Migration is opt-in through config changes

---

## Open Questions & Future Considerations

### Resolved During Planning
- ✅ Should receivers have sub-strategies? **No, flat enum is simpler**
- ✅ How to handle backward compatibility? **Default to Generic**
- ✅ Should ClaudeCli and Generic differ? **Only in command hardcoding**
- ✅ Error handling approach? **Clear messages with suggestions**

### For Future Iterations
- Should we support custom receiver plugins via dynamic loading?
- Should receivers have their own configuration sections?
- Do we need a receiver "capability" system (e.g., supports_stdin, supports_streaming)?
- Should we add a `--receiver` CLI flag to override config?

---

## Appendix

### Rust Stdin Best Practices
Based on Rust documentation and common patterns:

```rust
use std::io::Write;
use std::process::{Command, Stdio};

fn send_via_stdin(command: &str, args: &[String], prompt: &str) -> Result<()> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn command")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())
            .context("Failed to write to stdin")?;
        // stdin is automatically closed when dropped
    }

    let status = child.wait()
        .context("Failed to wait for child process")?;

    if !status.success() {
        anyhow::bail!("Command exited with status: {}", status);
    }

    Ok(())
}
```

### References
- Rust std::process documentation: https://doc.rust-lang.org/std/process/
- Shell ARG_MAX limits: https://www.in-ulm.de/~mascheck/various/argmax/
- Trait objects in Rust: https://doc.rust-lang.org/book/ch17-02-trait-objects.html

---

**Document Version**: 1.0
**Last Updated**: 2025-10-15
**Status**: Ready for Implementation
