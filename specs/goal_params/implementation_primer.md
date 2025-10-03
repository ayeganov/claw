# Goal Parameters Feature - LLM Implementation Primer

You are an expert Rust developer tasked with implementing a goal parameters feature for **claw**, a command-line tool that wraps LLM CLIs with goal-oriented, context-aware workflows.

## Project Overview

**claw** is a Rust CLI application that:
- Wraps underlying LLM tools (like `claude` or `gemini`)
- Provides goal-driven workflows via `prompt.yaml` configurations
- Uses cascading configuration: local (`./.claw/`) overrides global (`~/.config/claw/`)
- Executes context scripts (shell commands) before rendering prompts
- Uses the Tera templating engine for prompt rendering
- Supports file context inclusion via `--context` parameter

## Codebase Architecture

### File Structure
```
src/
├── main.rs              # Entry point, goal execution orchestration
├── cli.rs               # Clap argument parsing
├── config.rs            # Configuration loading and goal discovery
├── runner.rs            # LLM execution and context script running
├── context.rs           # File context management (--context flag)
├── goal_browser.rs      # Interactive TUI for goal selection
└── commands/
    ├── mod.rs
    └── add.rs           # Agent-assisted goal creation
```

### Key Data Structures

**Current Goal Schema (`config.rs:97-119`):**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct PromptConfig {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub context_scripts: HashMap<String, String>,
    pub prompt: String,
}
```

**Goal Loading (`config.rs:186-189`):**
```rust
pub struct LoadedGoal {
    pub config: PromptConfig,
    pub directory: PathBuf,
}
```

**CLI Structure (`cli.rs:5-36`):**
```rust
pub struct Cli {
    pub command: Option<Subcommands>,
    pub run_args: RunArgs,
}

pub struct RunArgs {
    pub goal_name: Option<String>,
    pub context: Vec<PathBuf>,           // --context flag
    pub recurse_depth: Option<usize>,     // --recurse_depth flag
    pub template_args: Vec<String>,       // Arbitrary args after goal name
}
```

### Current Argument Parsing

**Location:** `cli.rs:58-96`

Arguments are parsed by `parse_template_args()`:
- Supports `--key=value` and `--key value` formats
- Returns `HashMap<String, String>`
- Used in template rendering as `{{ Args.key }}`

**Important Note:** The current implementation uses `#[arg(last = true)]` to capture all trailing arguments, which was intended for use with the `--` separator pattern. This will need to change.

### Template Rendering Flow

**Location:** `main.rs:60-135`

1. Load goal configuration
2. Parse template args into HashMap
3. Create Tera context with Args
4. Render context scripts (which can use `{{ Args.* }}`)
5. Execute rendered scripts
6. Add script outputs to context as Context
7. Render main prompt with both Args and Context
8. Append file context if `--context` provided
9. Invoke LLM with rendered prompt

## Key Dependencies

- **clap 4.5.4** (with derive feature): CLI argument parsing
- **serde/serde_yaml**: YAML deserialization
- **tera 1.19.1**: Template rendering engine
- **anyhow 1.0.82**: Error handling
- **ratatui/crossterm**: TUI for goal browser
- **ignore, content_inspector**: File context management

## Current Patterns and Conventions

### Error Handling
- Use `anyhow::Result<T>` for fallible functions
- Use `.context()` or `.with_context()` to add helpful error messages
- Use `anyhow::bail!()` for early returns with errors

### Configuration Cascading
The `ConfigPaths` struct (config.rs:121-138) implements priority:
1. Local (`./.claw/`)
2. Global (`~/.config/claw/`)
3. Default (compiled-in)

### File Naming
- Snake_case for Rust files
- Kebab-case for directories and CLI arguments
- Underscore for struct fields, hyphen for CLI flags

### Testing Philosophy
- Integration tests preferred over unit tests where practical
- Error cases are tested explicitly
- User-facing messages are validated

## Implementation Task

**You must implement the Goal Parameters feature as specified in `specs/goal_params/spec.md`.**

### Critical Requirements Summary

1. **Extend `PromptConfig` schema** to include optional `parameters` field with:
   - name, description, required, type (optional), default (optional)

2. **Eliminate `--` separator** - make goal parameters natural CLI extensions:
   - Current: `claw review --context ./src -- --scope auth`
   - New: `claw review --context ./src --scope auth`

3. **Intelligent flag separation** - distinguish claw built-in vs goal parameters:
   - Built-in: `--context`, `--recurse_depth`, `--local`, `--global`, `--help`
   - Goal-defined: Anything in the goal's parameters section

4. **Parameter validation**:
   - Check required parameters before execution
   - Show helpful errors with format: "Missing required parameter: --scope (The scope of the review)"
   - Apply defaults for optional parameters

5. **Help system**:
   - Support `claw <goal> --help` (or `claw help <goal>` if more flexible)
   - Show grouped required/optional parameters
   - Display usage examples

6. **Add `list` command**:
   - `claw list`: Show all goals (local and global sections)
   - `claw list --local`: Show only local goals
   - `claw list --global`: Show only global goals
   - Display: CLI name, human name, description, parameter count

7. **Update `claw add` agent**:
   - Modify `prompts/add_meta_prompt.txt` to guide parameter definition
   - Generate parameters section in output YAML

8. **Backward compatibility**:
   - Goals without parameters section continue to work
   - Passing undefined args to parameterless goals is allowed

9. **Boolean handling**:
   - Support both `--verbose` and `--verbose=true/false`

10. **Template access**:
    - Keep existing `{{ Args.parameter }}` syntax

## Implementation Strategy

### Phase 1: Schema Extension
- Add new Rust structs to `config.rs`:
  ```rust
  pub struct GoalParameter {
      pub name: String,
      pub description: String,
      pub required: bool,
      pub param_type: Option<ParameterType>,
      pub default: Option<String>,
  }

  pub enum ParameterType {
      String,
      Number,
      Boolean,
  }
  ```
- Add `parameters: Vec<GoalParameter>` to `PromptConfig` with `#[serde(default)]`
- Write tests for YAML parsing

### Phase 2: Argument Parser Redesign
**This is the trickiest part.** Current approach with `#[arg(last = true)]` won't work.

**Recommended Approach:**
1. Define a static list of claw's built-in flags
2. Manually parse `std::env::args()` after goal name:
   - Separate built-in flags vs unknown flags
   - Keep built-in flags for claw's use
   - Pass unknown flags to parameter validator
3. Or use clap's `allow_external_subcommands` or dynamic subcommand approach
4. Load goal config early to know what parameters exist
5. Validate and parse goal parameters into `HashMap<String, String>`

**Key Challenge:** Need goal config to determine which flags belong to the goal, but need to parse args to know which goal to load.

**Solution:** Parse in two stages:
1. First pass: Extract goal name and claw built-in flags
2. Load goal configuration
3. Second pass: Parse remaining flags against goal's parameter definitions

### Phase 3: Parameter Validator Component
Create new module `src/validation.rs`:
```rust
pub struct ParameterValidator<'a> {
    parameters: &'a [GoalParameter],
}

impl<'a> ParameterValidator<'a> {
    pub fn validate(&self, args: HashMap<String, String>)
        -> Result<HashMap<String, String>, ValidationError>;

    pub fn format_error(&self, error: &ValidationError) -> String;
}
```

### Phase 4: Help Formatter
Create new module `src/help.rs`:
```rust
pub fn format_goal_help(goal: &LoadedGoal, goal_name: &str) -> String;
```

### Phase 5: List Command
Add to `cli.rs::Subcommands`:
```rust
List {
    #[arg(long)]
    local: bool,
    #[arg(long)]
    global: bool,
}
```

Implement handler in `main.rs` or new `commands/list.rs`.

### Phase 6: Update Agent
Modify `prompts/add_meta_prompt.txt` to add parameter definition step.

### Phase 7: Integration
Wire everything into `main.rs::run_goal()`:
1. After loading goal, check for `--help` flag
2. If present, format and display help, then exit
3. Otherwise, validate parameters
4. If validation fails, format and display error, then exit
5. If validation succeeds, proceed with current flow

## Important Implementation Notes

### 1. Clap Configuration Changes
The current `RunArgs` struct uses `#[arg(last = true)]` for `template_args`. This captures everything after the goal name, which was designed for the `--` separator pattern.

**This must change.** Options:
- Remove `template_args` field entirely, manually parse after goal name
- Use clap's flexibility to allow unknown flags (research required)
- Parse in multiple passes (extract goal name first, then reparse)

### 2. Parameter Name to Template Variable Mapping
If a parameter is named `with-tests` (with hyphen), should it be:
- `{{ Args.with-tests }}` (exact match)
- `{{ Args.with_tests }}` (convert to underscore)

**Recommendation:** Store with hyphens, but Tera may not support hyphens in variable names. Test this. If needed, convert hyphens to underscores in the HashMap keys.

### 3. Type Validation Complexity
The spec says:
> If clap library can dynamically parse based on parameter definitions, implement validation. Otherwise, use type information for documentation only.

**Recommendation:** Start with documentation-only. Clap's dynamic subcommand parsing is complex and may not support dynamic type validation easily. You can enhance this later.

### 4. Boolean Flag Handling
Clap supports:
- `#[arg(action = clap::ArgAction::SetTrue)]` for `--flag`
- But we need dynamic parameters

**Recommendation:** Parse manually. If parameter type is Boolean:
- `--verbose` → `Some(true)`
- `--verbose=true` → `Some(true)`
- `--verbose=false` → `Some(false)`
- Missing → Use default or None

### 5. Error Message Formatting
Follow existing patterns in the codebase. Example from `cli.rs:68`:
```
"Invalid template argument: '{}'. All template arguments must be flags starting with '--'."
```

Be consistent with this style: Clear, actionable, includes the problematic value.

### 6. Interactive Mode Complexity
The spec notes:
> Interactive mode parameter prompting is a big can of worms. Address separately.

**For this implementation:** When user selects a goal from TUI and it has required parameters, show an error message like:
```
Goal 'pr-notes' requires parameters. Run with --help to see details.
Example: claw pr-notes --scope authentication
```

Don't implement interactive parameter prompting yet.

### 7. Testing Checklist
Refer to section 5 of the spec. Key tests:
- Old goals without parameters work
- Goal with missing required param fails with clear message
- Goal with all params succeeds
- `--help` shows correct output
- `claw list` shows correct goals
- Boolean variations parse correctly
- Built-in flags don't conflict with goal params

### 8. Breaking Changes
This is a **breaking change**:
- Old: `claw goal -- --param value`
- New: `claw goal --param value`

Update documentation and examples in README.md after implementation.

## Code Style Guidelines

- Follow existing patterns in the codebase
- Use descriptive variable names (current code does this well)
- Add doc comments for public functions and structs
- Use `?` operator for error propagation
- Group imports: std, external crates, local modules
- Keep functions focused and single-purpose
- Extract complex logic into helper functions

## Validation Before Completion

Before considering this feature complete, verify:

1. ✅ All 10 test scenarios from spec section 5 pass
2. ✅ Backward compatibility: old goals work
3. ✅ `claw list` displays correctly
4. ✅ `claw <goal> --help` works
5. ✅ Error messages are clear and helpful
6. ✅ No panics or unwraps in happy path
7. ✅ Parameters with defaults work correctly
8. ✅ Boolean flags work in all variations
9. ✅ Built-in and goal parameters coexist
10. ✅ Agent generates valid parameter sections

## Reference Documentation

**Full Specification:** `specs/goal_params/spec.md`

This document contains:
- Detailed requirements (FR1-FR10, NFR1-NFR4)
- Complete architecture and component descriptions
- Implementation plan with task dependencies
- Full testing strategy
- Example goal with parameters

**Read the specification carefully before beginning implementation.**

---

## Getting Started

1. Read `specs/goal_params/spec.md` in full
2. Start with Task 1: Schema Extension (independent)
3. Write tests for schema parsing before moving on
4. Tackle Task 2-3: Argument Parser (most complex part)
5. Implement remaining tasks following dependency graph
6. Run full test suite
7. Update README.md with examples

**Remember:** You're not implementing everything at once. Follow the task breakdown in the spec, complete one task fully before moving to the next, and write tests as you go.

Good luck! 🐾
