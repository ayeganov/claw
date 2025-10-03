# Goal Parameters Feature Specification

## 1. Overview

### Problem Statement
Currently, claw goals can accept arbitrary arguments passed after `--` (e.g., `claw review --context ./src -- --scope authentication`), but there is no way to:
- Discover what parameters a goal accepts
- Enforce required parameters
- Provide helpful error messages for missing parameters
- Display parameter documentation

### Goals
1. Add a formal parameter definition system to goal specifications
2. Validate required parameters before goal execution
3. Provide clear help and documentation for goal parameters
4. Eliminate the awkward `--` separator, making parameters natural CLI extensions
5. Add goal listing and discovery capabilities

### Target Users
- **End Users:** Developers using claw to run goals with clear parameter guidance
- **Goal Authors:** Developers creating and maintaining goals with documented parameters

### Success Criteria
- Goals can define required and optional parameters with descriptions
- Running a goal without required parameters shows helpful error messages
- Users can view available goals and their parameters
- Tests verify parameter validation works correctly
- Backward compatibility: goals without parameters section continue to work
- Natural CLI syntax without `--` separator

## 2. Requirements

### Functional Requirements

#### FR1: Extended Goal Schema
- Add optional `parameters` section to `prompt.yaml`
- Each parameter must support:
  - `name` (required): Parameter identifier (e.g., "scope")
  - `description` (required): Human-readable explanation
  - `required` (required): Boolean indicating if parameter is mandatory
  - `type` (optional): Type hint (string, number, boolean) - primarily for documentation
  - `default` (optional): Default value for optional parameters

#### FR2: Parameter Validation
- Before executing a goal, validate that all required parameters are provided
- If validation fails, display:
  - Clear error message
  - List of missing required parameters with descriptions
  - Example of correct usage
- Format: "Missing required parameter: --scope (description: The scope of the review)"

#### FR3: Parameter Help System
- Support viewing goal parameters without execution
- Flexible approach: `claw review --help` or `claw help review` (whichever is more maintainable)
- Help output must show:
  - Goal name and description
  - Required parameters (grouped)
  - Optional parameters (grouped)
  - Parameter descriptions, types, and defaults
  - Usage examples

#### FR4: Goal Listing Command
- Add `claw list` command to display all available goals
- Display format per goal:
  - CLI name (e.g., "review")
  - Human-readable name (from `name` field)
  - Description
  - Parameter count (e.g., "2 required, 1 optional")
- Separate sections for local (`./.claw/`) and global (`~/.config/claw/`) goals
- Support filtering:
  - `claw list --local`: Show only local goals
  - `claw list --global`: Show only global goals
  - `claw list`: Show both, separated by sections

#### FR5: Natural CLI Syntax
- Eliminate requirement for `--` separator
- Parse parameters naturally: `claw review --context ./src --scope authentication`
- Intelligently distinguish between:
  - Claw built-in flags (e.g., `--context`, `--local`, `--global`)
  - Goal-defined parameters (e.g., `--scope`, `--lang`)
- Both types can coexist in the same command

#### FR6: Boolean Parameter Handling
- Support flag-style: `--verbose` (presence implies true)
- Support explicit values: `--verbose=true` or `--verbose=false`

#### FR7: Type Coercion
- If clap library can dynamically parse based on parameter definitions, implement validation
- Otherwise, use type information for documentation only
- Priority: Keep implementation simple over strict validation

#### FR8: Template Access
- Maintain existing `{{ Args.parameter_name }}` syntax in prompt templates
- No breaking changes to template engine

#### FR9: Agent-Assisted Parameter Definition
- Update `claw add` command to help define parameters interactively
- After gathering goal metadata and context scripts, ask: "Does this goal need parameters?"
- Guide user through defining each parameter:
  - Name
  - Description
  - Required vs optional
  - Type (optional)
  - Default value (for optional params)

#### FR10: Backward Compatibility
- **Breaking Change Accepted:** Remove support for `--` separator syntax
- Goals without `parameters` section remain valid (all parameters optional/undefined)
- Passing undefined parameters to a goal without parameters section is allowed (no error)

### Non-Functional Requirements

#### NFR1: Performance
- Parameter parsing and validation must not significantly impact startup time
- Goal listing should be fast even with many goals

#### NFR2: Maintainability
- Leverage existing clap library for argument parsing where possible
- Keep schema extensions minimal and well-documented
- Clear separation between claw's built-in CLI and goal parameters

#### NFR3: User Experience
- Error messages must be clear and actionable
- Help output must be well-formatted and easy to read
- Examples should be included in help text

#### NFR4: Testing
- Unit tests for parameter schema parsing
- Integration tests for:
  - Running goal with missing required parameters (should fail)
  - Running goal with all required parameters (should succeed)
  - Running old goals without parameters section (should succeed)
  - Help/list commands output

### Dependencies and Prerequisites
- Rust clap library (already in use)
- tera templating engine (already in use)
- YAML parser (serde_yaml, already in use)

## 3. Architecture & Design

### High-Level Architecture

```
User Command
    ↓
CLI Parser (clap)
    ↓
[Built-in flags separated] → Context, Local/Global flags, etc.
    ↓
Goal Resolver
    ↓
Goal Loader → Load prompt.yaml
    ↓
Parameter Validator
    ↓
[If --help] → Parameter Help Formatter → Display & Exit
[If validation fails] → Error Formatter → Display & Exit
[If validation passes] → Context Script Executor → Prompt Renderer → LLM Invocation
```

### Key Components

#### 3.1 Goal Schema Extension (`prompt.yaml`)

**New Schema Structure:**
```yaml
name: "Pull Request Notes"
description: "Generates PR notes based on changes in the current branch."

# NEW: Optional parameters section
parameters:
  - name: scope
    description: "The scope or focus area of the PR"
    required: true
    type: string

  - name: format
    description: "Output format for the notes"
    required: false
    type: string
    default: "markdown"

  - name: verbose
    description: "Include detailed commit information"
    required: false
    type: boolean
    default: false

context_scripts:
  branch_diff: "git diff main...HEAD"
  file_list: "git diff --name-only main...HEAD"

prompt: |
  Generate PR notes for scope: {{ Args.scope }}
  Format: {{ Args.format }}
  Verbose: {{ Args.verbose }}

  {{ Context.branch_diff }}
```

**Rust Data Structure:**
```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct GoalParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    #[serde(default)]
    pub param_type: Option<ParameterType>,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
}

#[derive(Debug, Deserialize)]
pub struct GoalConfig {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Vec<GoalParameter>,
    #[serde(default)]
    pub context_scripts: HashMap<String, String>,
    pub prompt: String,
}
```

#### 3.2 Parameter Validator Component

**Responsibilities:**
- Parse command-line arguments into goal parameters
- Separate built-in claw flags from goal parameters
- Validate required parameters are present
- Apply default values for missing optional parameters
- Generate helpful error messages

**Interface:**
```rust
pub struct ParameterValidator {
    goal_config: GoalConfig,
}

impl ParameterValidator {
    pub fn validate(&self, args: &ParsedArgs) -> Result<HashMap<String, String>, ValidationError>;
    pub fn get_missing_required(&self, args: &ParsedArgs) -> Vec<&GoalParameter>;
}

pub struct ValidationError {
    pub missing_params: Vec<GoalParameter>,
    pub goal_name: String,
}
```

#### 3.3 CLI Argument Parser

**Challenge:** Dynamically parse arguments based on loaded goal configuration

**Approach:**
1. First pass: Parse built-in claw flags (`--context`, `--local`, `--global`, etc.)
2. Load goal configuration
3. Second pass: Parse remaining arguments as goal parameters
4. Validate against goal's parameter definitions

**Implementation Strategy:**
- Use clap's `allow_external_subcommands` or similar flexibility
- Or manually parse remaining args after extracting built-in flags
- Map parsed values into `HashMap<String, String>` for template rendering

#### 3.4 Help Formatter Component

**Responsibilities:**
- Format goal parameter information for display
- Group parameters by required vs optional
- Generate usage examples

**Output Format:**
```
Goal: Pull Request Notes (pr-notes)
Description: Generates PR notes based on changes in the current branch.

Required Parameters:
  --scope <string>
      The scope or focus area of the PR

Optional Parameters:
  --format <string>  (default: "markdown")
      Output format for the notes

  --verbose <boolean>  (default: false)
      Include detailed commit information

Usage Examples:
  claw pr-notes --scope authentication
  claw pr-notes --scope api --format json --verbose
  claw pr-notes --context ./src --scope frontend
```

#### 3.5 Goal Lister Component

**Responsibilities:**
- Scan local and global goal directories
- Load minimal metadata (name, description, parameters)
- Format output with sections

**Output Format:**
```
Local Goals (./.claw/):
  my-project-review - Project Code Review
    Specialized review for this project's architecture
    Parameters: 2 required, 1 optional

Global Goals (~/.config/claw/):
  code-review - Code Review
    General code review with best practices
    Parameters: 0 required, 2 optional

  pr-notes - Pull Request Notes
    Generates PR notes based on changes in the current branch
    Parameters: 1 required, 2 optional
```

#### 3.6 Updated `claw add` Agent Prompt

**Additions to `add_meta_prompt.txt`:**

After step 2.e (writing the main prompt), add:

```
3. **Define Parameters (Optional):**
   - Ask the user: "Does this goal need any parameters to customize its behavior?"
   - If yes, for each parameter:
     a. Ask for the parameter name (e.g., "scope", "format")
     b. Ask for a description
     c. Ask if it's required or optional
     d. If optional, ask if there should be a default value
     e. Optionally ask for type (string, number, boolean)
   - Add the `parameters` section to the `prompt.yaml`
   - Update the `prompt` template to use `{{ Args.parameter_name }}` where appropriate
```

### Data Structures and Types

See section 3.1 for Rust structures.

### Integration Points

1. **CLI Entry Point (`main.rs`):**
   - Add `list` subcommand
   - Modify goal execution path to include parameter validation
   - Add `--help` flag handling per goal

2. **Goal Loading (`goal_loader.rs` or similar):**
   - Parse `parameters` section from YAML
   - Expose parameter definitions to validator

3. **Template Rendering:**
   - No changes needed - continue using `{{ Args.* }}` syntax
   - Args map now populated from validated parameters

4. **Agent System (`claw add`):**
   - Update meta-prompt template to include parameter definition step
   - Generate parameter section in output YAML

## 4. Implementation Plan

### Task Breakdown

#### Task 1: Extend Goal Schema (Independent)
- Add `GoalParameter` struct and `ParameterType` enum
- Update `GoalConfig` to include `parameters: Vec<GoalParameter>`
- Update YAML deserialization
- Write unit tests for schema parsing

**Dependencies:** None
**Can be parallelized:** Yes

#### Task 2: Implement Parameter Validator (Depends on Task 1)
- Create `ParameterValidator` component
- Implement validation logic
- Implement default value application
- Write unit tests for validation scenarios

**Dependencies:** Task 1
**Can be parallelized:** No

#### Task 3: Update CLI Argument Parser (Depends on Task 2)
- Modify clap configuration to separate built-in vs goal params
- Implement two-pass parsing strategy
- Parse goal parameters into HashMap
- Write integration tests

**Dependencies:** Task 2
**Can be parallelized:** No

#### Task 4: Implement Help Formatter (Depends on Task 1)
- Create help output formatter
- Implement grouping logic (required vs optional)
- Generate usage examples
- Add `--help` flag handling to CLI
- Write unit tests for formatting

**Dependencies:** Task 1
**Can be parallelized:** With Task 2 (both depend only on Task 1)

#### Task 5: Implement Goal Lister (Depends on Task 1)
- Create `list` subcommand in CLI
- Implement goal scanning for local/global directories
- Format output with sections
- Add `--local` and `--global` filters
- Write integration tests

**Dependencies:** Task 1
**Can be parallelized:** With Tasks 2 and 4

#### Task 6: Update Agent Meta-Prompt (Depends on Task 1)
- Update `prompts/add_meta_prompt.txt`
- Add parameter definition conversation flow
- Include parameter section in generated YAML
- Test with `claw add` command

**Dependencies:** Task 1
**Can be parallelized:** With Tasks 2, 4, and 5

#### Task 7: Integration and Error Handling (Depends on Tasks 2, 3, 4)
- Wire parameter validator into goal execution flow
- Implement error message formatting
- Add validation checks before LLM invocation
- Write end-to-end integration tests

**Dependencies:** Tasks 2, 3, 4
**Can be parallelized:** No

#### Task 8: Documentation and Examples (Independent)
- Update README.md with parameter examples
- Create example goals with parameters
- Document breaking changes (removal of `--`)
- Add migration guide for existing goals

**Dependencies:** None (but should be final step)
**Can be parallelized:** With testing tasks

#### Task 9: Testing (Depends on All Implementation Tasks)
- Test: goal with missing required params (should fail with clear message)
- Test: goal with all required params (should succeed)
- Test: goal without parameters section (should succeed)
- Test: old goals continue to work
- Test: `claw list` output correctness
- Test: `claw <goal> --help` output correctness
- Test: boolean flag variations
- Test: default value application

**Dependencies:** Tasks 1-7
**Can be parallelized:** No

### Sequence and Dependencies

```
Task 1 (Schema Extension)
    ↓
    ├─→ Task 2 (Validator) ─→ Task 3 (CLI Parser) ─→ Task 7 (Integration)
    ├─→ Task 4 (Help Formatter) ────────────────────→ Task 7 (Integration)
    ├─→ Task 5 (Goal Lister)
    └─→ Task 6 (Agent Update)

Task 8 (Documentation) - Can start anytime, finalize last
Task 9 (Testing) - After all implementation tasks
```

### Parallelization Opportunities

- **Phase 1:** Task 1 (foundational)
- **Phase 2:** Tasks 2, 4, 5, 6 (parallel - all depend only on Task 1)
- **Phase 3:** Task 3 (depends on Task 2)
- **Phase 4:** Task 7 (integration - depends on 2, 3, 4)
- **Phase 5:** Tasks 8 and 9 (documentation and testing)

## 5. Testing Strategy

### Test Scenarios

#### TS1: Missing Required Parameters
- **Setup:** Goal with one required parameter (`--scope`)
- **Action:** Run `claw goal-name` without `--scope`
- **Expected:** Error message showing missing parameter with description and example

#### TS2: All Required Parameters Provided
- **Setup:** Goal with required parameters
- **Action:** Run `claw goal-name --scope auth --lang rust`
- **Expected:** Goal executes successfully, parameters available in template

#### TS3: Backward Compatibility (No Parameters Section)
- **Setup:** Old goal without `parameters` section in YAML
- **Action:** Run goal with or without arguments
- **Expected:** Goal executes successfully

#### TS4: Optional Parameters with Defaults
- **Setup:** Goal with optional param `format` (default: "markdown")
- **Action:** Run goal without specifying `--format`
- **Expected:** Default value used in template

#### TS5: Boolean Flag Variations
- **Setup:** Goal with boolean parameter `--verbose`
- **Action:** Test `--verbose`, `--verbose=true`, `--verbose=false`
- **Expected:** All variations parse correctly

#### TS6: Parameter Help Display
- **Setup:** Goal with mixed required/optional parameters
- **Action:** Run `claw goal-name --help`
- **Expected:** Well-formatted help with grouping and examples

#### TS7: Goal Listing
- **Setup:** Multiple local and global goals
- **Action:** Run `claw list`, `claw list --local`, `claw list --global`
- **Expected:** Correct goals shown in appropriate sections

#### TS8: Built-in vs Goal Parameter Separation
- **Setup:** Goal with parameter `--format`
- **Action:** Run `claw goal-name --context ./src --format json`
- **Expected:** `--context` handled by claw, `--format` passed to goal

#### TS9: Undefined Parameters (No Parameters Section)
- **Setup:** Goal without parameters section
- **Action:** Run `claw goal-name --random-arg value`
- **Expected:** No error, argument available in template

#### TS10: Agent Parameter Generation
- **Setup:** Use `claw add new-goal`
- **Action:** Follow prompts to define parameters
- **Expected:** Generated `prompt.yaml` has correct parameters section

### Acceptance Criteria

✅ All test scenarios pass
✅ Existing goals without parameters continue to work
✅ Help messages are clear and actionable
✅ Error messages guide users to correct usage
✅ Goal listing shows accurate information
✅ No significant performance regression
✅ Documentation updated with examples
✅ Breaking changes clearly communicated

### Edge Cases

- Goal with 10+ parameters (help formatting)
- Parameter name conflicts with built-in flags
- Special characters in parameter values
- Empty parameter values
- Parameter names with hyphens vs underscores
- Very long descriptions (word wrapping)
- Goals in deeply nested directories

---

## Appendix: Example Goal with Parameters

```yaml
# ~/.config/claw/goals/generate-component/prompt.yaml

name: "Generate Component"
description: "Generates a new software component with boilerplate code"

parameters:
  - name: name
    description: "Name of the component to generate"
    required: true
    type: string

  - name: type
    description: "Type of component (e.g., React, Vue, Class)"
    required: true
    type: string

  - name: with-tests
    description: "Include unit test boilerplate"
    required: false
    type: boolean
    default: "false"

  - name: style
    description: "Coding style to follow"
    required: false
    type: string
    default: "default"

context_scripts:
  project_structure: "ls -R ./src"

prompt: |
  You are a code generation expert.

  Generate a {{ Args.type }} component named {{ Args.name }}.
  Style guide: {{ Args.style }}
  Include tests: {{ Args.with-tests }}

  Here is the current project structure:
  {{ Context.project_structure }}

  Please provide the complete implementation.
```

**Usage:**
```bash
# View help
claw generate-component --help

# Run with required params
claw generate-component --name UserProfile --type React

# Run with all params
claw generate-component --name UserProfile --type React --with-tests --style airbnb

# Run with context
claw generate-component --context ./src --name LoginForm --type React
```
