# LLM Implementation Primer: Context Management 2.0

You are an expert Rust developer tasked with implementing a new feature for **claw**, a command-line utility that wraps LLM CLI tools with goal-oriented, context-aware workflows.

## About Claw

Claw is a Rust-based CLI tool that:
- Acts as an intelligent wrapper around LLM command-line tools (e.g., `claude`, `gemini-cli`)
- Provides goal-oriented workflows where users can run predefined prompts (goals)
- Executes shell scripts to gather context before invoking the LLM
- Uses Tera templating to inject context and arguments into prompts
- Supports cascading configuration (local `.claw/` and global `~/.config/claw/`)

## Current Architecture

### Key Files

**`src/config.rs`**:
- Defines `ClawConfig` for LLM command configuration
- Defines `PromptConfig` for goal definitions
- Handles configuration cascade (local → global → defaults)
- Provides goal discovery and loading functionality

**`src/runner.rs`**:
- Executes context scripts defined in goals
- Renders prompts using Tera templates
- Invokes the configured LLM command
- Current flow: execute scripts → render prompt → run LLM

**`src/cli.rs`** (implied):
- Handles command-line argument parsing
- Supports subcommands like `add`, `pass`, and goal execution

**`src/main.rs`** (implied):
- Application entry point
- Orchestrates CLI parsing and command dispatch

### Current Goal Workflow

1. User runs: `claw code-review --scope="authentication"`
2. Claw loads `code-review` goal from `.claw/goals/` or `~/.config/claw/goals/`
3. Executes any `context_scripts` defined in `prompt.yaml` (e.g., `git diff --staged`)
4. Renders prompt template with script outputs and CLI args
5. Passes final prompt to underlying LLM command

## Your Task

Implement the **Context Management 2.0** feature that allows users to pass arbitrary files and directories as context to any goal via CLI parameters.

### Key Requirements

1. **CLI Parameters**: Add `--context` (file/directory list) and `--recurse_depth` (recursion control)
2. **File Discovery**: Recursively traverse directories with configurable depth limits
3. **Safety & Filtering**: Enforce file size limits, per-directory file count limits, and exclusion patterns
4. **Error Handling**: Support three modes (strict/flexible/ignore) for handling errors
5. **Formatting**: Generate clean markdown output in Repomix-inspired format
6. **Integration**: Append formatted file context to rendered prompts deterministically
7. **Configuration**: Extend `claw.yaml` with context management settings

### Implementation Approach

- **Follow the specification's implementation plan** with its phased approach
- **Maintain backward compatibility** with existing goals
- **Use idiomatic Rust**: leverage type safety, Result types, and clear error handling
- **Add comprehensive tests**: unit tests for core logic, integration tests for end-to-end flow
- **Keep concerns separated**: create a dedicated `src/context.rs` module
- **Consider dependencies**: `walkdir` for directory traversal, `ignore` crate for `.gitignore` support

### Code Style & Conventions

Based on the existing codebase:
- Use `anyhow::Result` for error handling
- Use `serde::Deserialize` for configuration structs
- Provide sensible defaults via default functions (e.g., `fn default_max_file_size_kb() -> u64`)
- Use clear, descriptive variable names
- Add doc comments for public types and functions
- Follow existing patterns for configuration cascade and module structure

### Important Notes

- **Do NOT break existing functionality** - all current goals must continue to work
- **The context section is appended to the rendered prompt**, not injected via Tera variables
- **File reading should be safe**: validate sizes, detect binary files, handle UTF-8 errors
- **Performance matters**: don't read entire large directory trees into memory at once
- **Error messages should be actionable**: tell users exactly what went wrong and how to fix it

### Questions to Consider

As you implement, think about:
- How to efficiently count files per directory without reading contents?
- Should symlinks be followed or skipped?
- How to make the "flexible" error mode's user prompt clear and helpful?
- What's the best way to generate the directory tree visualization?
- Should the formatted context section have a maximum total size?

---

## The Specification Follows Below

