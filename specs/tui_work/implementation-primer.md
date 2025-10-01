# Implementation Primer: Interactive Goal Browser

You are an expert Rust engineer tasked with implementing a new feature for the `claw` CLI tool. You will be provided with a detailed specification document that describes the feature requirements, architecture, and implementation plan.

## Your Role

You are a collaborative implementation partner who will:
- Follow the provided specification closely
- Write high-quality, idiomatic Rust code
- Ask clarifying questions when requirements are ambiguous
- Propose improvements or alternatives when you see issues
- Implement tests alongside features
- Integrate seamlessly with the existing codebase

## Context: The Claw Project

`claw` is a goal-driven, context-aware wrapper for LLM CLIs. It uses:
- **Configuration system**: Cascading local (`.claw/`) and global (`~/.config/claw/`) configs
- **Goals**: User-defined prompt templates with context scripts stored in `goals/` subdirectories
- **Current tech stack**:
  - `clap` for CLI parsing
  - `tera` for templating
  - `serde`/`serde_yaml` for config parsing
  - `dialoguer` for simple prompts (being replaced)
  - `anyhow` for error handling

The codebase is well-structured with clear separation of concerns:
- `src/main.rs`: Entry point, orchestration
- `src/config.rs`: Configuration loading and goal discovery
- `src/runner.rs`: Script execution and LLM invocation
- `src/cli.rs`: Command-line argument parsing
- `src/commands/`: Subcommands (currently just `add`)

## The Specification

# Interactive Goal Browser - Specification

## 1. Overview

### Problem Statement
The current interactive goal selection in `claw` uses a simple single-list menu via `dialoguer::Select`. Goals from local and global sources are intermixed in one list with only a small `(local)` or `(global)` indicator that is not visually prominent. Users cannot preview the content of a goal's `prompt.yaml` before selecting it, making it difficult to remember what each goal does or distinguish between similarly-named goals from different sources.

### Goals
- Create a visually appealing, dual-panel TUI for browsing and selecting goals
- Clearly separate local and global goals into distinct sections
- Allow users to preview goal content (prompt.yaml) before selection
- Provide intuitive keyboard-based navigation
- Enhance the user experience with a branded ASCII art logo on startup

### Target Users
End users running `claw` interactively without command-line arguments (i.e., when no goal name is provided).

### Success Criteria
- Users can easily distinguish between local and global goals
- Users can navigate between panels and goals with keyboard shortcuts
- Users can view full goal content in a scrollable preview mode
- The interface is visually pleasing and polished
- All functionality is covered by unit tests where feasible

---

## 2. Requirements

### Functional Requirements

#### FR1: ASCII Art Logo Display
- Display a large "Claw" ASCII art logo when the interactive browser starts
- Logo height should be approximately 48pt (roughly 12-15 terminal lines)
- Logo should use multiple colors for visual appeal
- Logo should be centered or left-aligned consistently

#### FR2: Dual-Panel Layout
- Split the screen into two distinct panels: left for local goals, right for global goals
- Each panel displays a list of goals from its respective source
- Clearly indicate which panel is currently active (via highlighting, border, or color)
- Each goal entry shows: name and description (if available)

#### FR3: Keyboard Navigation
- **Tab**: Switch focus between local and global panels
- **Arrow keys (↑/↓) OR hjkl (vim-style)**: Navigate up/down within the active panel
- **'v' key**: Enter view mode to display the selected goal's `prompt.yaml` content
- **'Esc' key**:
  - If in view mode, exit back to panel selection
  - If in panel selection mode, quit the application (or prompt for confirmation)
- **Enter/Return**: Select the currently highlighted goal and proceed with execution

#### FR4: Goal Preview Mode
- When 'v' is pressed, display the full content of the selected goal's `prompt.yaml`
- Preview should be displayed in a scrollable view (support long files)
- Optionally apply syntax highlighting or formatting for YAML content
- Display filename/path at the top of the preview
- Pressing 'Esc' returns to the dual-panel view

#### FR5: Goal Execution
- When a goal is selected (via Enter), return the goal name to the main application logic
- Integration should be seamless with existing `run_goal()` function in `src/main.rs`

### Non-Functional Requirements

#### NFR1: Visual Polish
- Interface should be clean, readable, and aesthetically pleasing
- Use colors and borders effectively to delineate sections
- Ensure compatibility with standard terminal color schemes

#### NFR2: Terminal Compatibility
- Must work in standard terminal emulators (xterm, iTerm2, Windows Terminal, etc.)
- Gracefully handle terminal resize events
- Minimum terminal size should be documented (e.g., 80x24)

#### NFR3: Maintainability
- Code should be modular and separated into logical units
- Unit tests should cover core navigation and state logic
- Clear documentation of keybindings in code and help text

#### NFR4: Performance
- Goal list rendering and navigation should be instant (< 50ms response to keypress)
- View mode should handle large prompt.yaml files (up to several KB) smoothly

### Dependencies and Prerequisites
- Replace `dialoguer` with `ratatui` (modern TUI framework for Rust)
- Additional dependencies as needed:
  - `crossterm` (backend for ratatui, terminal manipulation)
  - Possibly `syntect` or similar for YAML syntax highlighting (optional enhancement)

---

## 3. Architecture & Design

### High-Level Architecture

```
┌─────────────────────────────────────────────┐
│          src/main.rs (existing)             │
│  - Entry point                              │
│  - Goal discovery (find_all_goals)          │
│  - Calls new TUI module when no goal given  │
└──────────────────┬──────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────┐
│      src/goal_browser.rs (NEW MODULE)       │
│  - TUI initialization                       │
│  - Event loop and state management          │
│  - Rendering logic                          │
│  - Returns selected goal name               │
└─────────────────────────────────────────────┘
```

### Key Components

#### Component 1: `GoalBrowserApp` (State Manager)
**Responsibilities:**
- Maintain application state (current panel, selected indices, view mode state)
- Handle transitions between modes (panel selection, view mode)
- Store goal lists (local and global)

**State Fields:**
```rust
struct GoalBrowserApp {
    local_goals: Vec<DiscoveredGoal>,
    global_goals: Vec<DiscoveredGoal>,
    active_panel: Panel, // enum: Local | Global
    local_selected: usize,
    global_selected: usize,
    mode: AppMode, // enum: Selection | ViewMode
    view_scroll: usize, // scroll offset in view mode
    view_content: Option<String>, // cached prompt.yaml content
}

enum Panel {
    Local,
    Global,
}

enum AppMode {
    Selection,
    ViewMode,
}
```

#### Component 2: `render_ui()` (Rendering Engine)
**Responsibilities:**
- Draw the logo at startup (or persistently at top)
- Render dual-panel layout with current state
- Highlight active panel and selected goal
- Render view mode overlay/screen

**Key Functions:**
- `render_logo(frame, area)`: Draw ASCII art logo
- `render_panels(frame, app_state)`: Draw left and right panels with goal lists
- `render_view_mode(frame, app_state)`: Draw scrollable prompt.yaml preview

#### Component 3: `handle_input()` (Event Handler)
**Responsibilities:**
- Process keyboard input events
- Update `GoalBrowserApp` state based on input
- Handle navigation (tab, arrows, hjkl)
- Handle mode transitions (v, esc)
- Handle selection (enter)

**Key Logic:**
```rust
fn handle_input(event: KeyEvent, app: &mut GoalBrowserApp) -> ControlFlow {
    match app.mode {
        AppMode::Selection => {
            match event.code {
                KeyCode::Tab => toggle_panel(app),
                KeyCode::Up | KeyCode::Char('k') => move_up(app),
                KeyCode::Down | KeyCode::Char('j') => move_down(app),
                KeyCode::Char('v') => enter_view_mode(app),
                KeyCode::Enter => return ControlFlow::Select,
                KeyCode::Esc => return ControlFlow::Quit,
                _ => {}
            }
        }
        AppMode::ViewMode => {
            match event.code {
                KeyCode::Up | KeyCode::Char('k') => scroll_up(app),
                KeyCode::Down | KeyCode::Char('j') => scroll_down(app),
                KeyCode::Esc => exit_view_mode(app),
                _ => {}
            }
        }
    }
    ControlFlow::Continue
}
```

#### Component 4: `run_goal_browser()` (Public API)
**Responsibilities:**
- Entry point called from `src/main.rs`
- Initialize terminal, TUI backend, and app state
- Run event loop
- Clean up terminal on exit
- Return selected goal name or error

**Function Signature:**
```rust
pub fn run_goal_browser(goals: Vec<DiscoveredGoal>) -> Result<String>
```

### Integration Points with Existing Code

**In `src/main.rs`, replace this block:**
```rust
// Current code (lines ~70-90)
let selection = Select::with_theme(&ColorfulTheme::default())
    .with_prompt("Choose a goal to run")
    .items(&items)
    .default(0)
    .interact()?;

let selected_goal_name = &goals[selection].name;
```

**With:**
```rust
let selected_goal_name = goal_browser::run_goal_browser(goals)?;
```

### Data Structures and Types

#### Input Type (Already exists)
```rust
// From src/config.rs
pub struct DiscoveredGoal {
    pub name: String,
    pub source: GoalSource, // Local or Global
    pub config: PromptConfig,
}
```

#### Output Type
```rust
// Return type: Result<String> where String is the goal name
```

### ASCII Art Logo Design
- Logo text: "Claw"
- Style: Large block letters using box-drawing characters or ASCII art fonts
- Colors: Use `ratatui` color styling, e.g.:
  - 'C' in cyan
  - 'l' in magenta
  - 'a' in yellow
  - 'w' in green
- Height: ~12-15 terminal lines (approximates 48pt)
- Can use tools like `figlet` with fonts like "big" or "slant" for initial generation

---

## 4. Implementation Plan

### Task 1: Set Up Dependencies
- Add `ratatui` and `crossterm` to `Cargo.toml`
- Remove or keep `dialoguer` (may still be useful for other prompts)
- Verify compatibility with existing dependencies

### Task 2: Create Goal Browser Module Skeleton
- Create `src/goal_browser.rs`
- Define `GoalBrowserApp` struct and state enums
- Implement `run_goal_browser()` stub function
- Add basic terminal initialization and cleanup

### Task 3: Implement State Management
- Implement state transitions (panel switching, selection movement)
- Add helper functions: `move_up()`, `move_down()`, `toggle_panel()`, etc.
- Write unit tests for state logic

### Task 4: Implement Rendering - Logo
- Create `render_logo()` function
- Design and embed ASCII art logo with colors
- Test logo display on startup

### Task 5: Implement Rendering - Dual Panels
- Create `render_panels()` function
- Implement left panel (local goals) and right panel (global goals)
- Add highlighting for active panel and selected goal
- Handle empty goal lists gracefully (e.g., "No local goals found")

### Task 6: Implement Input Handling - Selection Mode
- Implement `handle_input()` for selection mode
- Handle tab, arrows, hjkl, enter, esc
- Wire up to state management functions

### Task 7: Implement View Mode - Content Loading
- Implement `enter_view_mode()` to load and cache prompt.yaml content
- Read file from goal directory using existing `LoadedGoal` logic
- Handle errors gracefully (e.g., file not found)

### Task 8: Implement View Mode - Rendering
- Create `render_view_mode()` function
- Display prompt.yaml content in a scrollable widget
- Show filename/path at top
- Implement scroll state management

### Task 9: Implement View Mode - Input Handling
- Handle up/down/hjkl for scrolling
- Handle esc to exit view mode
- Add page up/down for faster scrolling (optional)

### Task 10: Integration with Main
- Update `src/main.rs` to call `run_goal_browser()`
- Remove old `dialoguer::Select` code
- Test end-to-end flow

### Task 11: Polish and Edge Cases
- Handle terminal resize events
- Add help text (e.g., footer showing available keybindings)
- Test with edge cases: empty lists, very long goal names, very long prompts
- Ensure consistent behavior across platforms

### Task 12: Documentation and Testing
- Document keybindings in code comments and module-level docs
- Write integration tests if feasible
- Update README or user documentation with new interactive mode features

### Task Dependencies
- Tasks 1-2 are foundational (must be done first)
- Tasks 3-9 can be partially parallelized but have logical order:
  - State (3) before rendering (4-5) and input (6)
  - Selection mode (4-6) before view mode (7-9)
- Tasks 10-12 are final integration and polish

---

## 5. Testing Strategy

### Test Scenarios

#### Unit Tests (State Logic)
- Test `move_up()` at boundaries (top of list, empty list)
- Test `move_down()` at boundaries (bottom of list, empty list)
- Test `toggle_panel()` switches active panel correctly
- Test `enter_view_mode()` loads correct goal's prompt.yaml
- Test `exit_view_mode()` returns to correct panel state
- Test scroll boundaries in view mode

#### Integration Tests
- Test full selection flow: start → navigate → select → return goal name
- Test view mode flow: start → navigate → view → scroll → exit → select
- Test quit flow: start → esc → quit

#### Manual Testing (UI/UX)
- Visual inspection of logo rendering
- Panel layout and alignment across different terminal sizes
- Color and highlighting visibility in different terminal themes
- Responsiveness of navigation (should feel instant)
- Scrolling smoothness in view mode
- Behavior on terminal resize

### Edge Cases
- No local goals (only global)
- No global goals (only local)
- No goals at all (should not reach browser, handled in main)
- Very long goal names (truncation or wrapping)
- Very long descriptions (truncation)
- Very large prompt.yaml files (scrolling performance)
- Terminal too small (minimum size validation)

### Acceptance Criteria
- All keybindings work as specified
- Panels clearly indicate local vs global goals
- Active panel and selected goal are visually obvious
- View mode displays full prompt.yaml content with scrolling
- Logo displays correctly on startup
- No crashes or panics on normal usage
- Unit tests pass for state logic
- Manual testing confirms pleasant user experience

---

## 6. Future Enhancements (Out of Scope)
- Syntax highlighting for YAML in view mode
- Search/filter functionality
- Mouse support for clicking goals
- Customizable keybindings
- Displaying additional goal metadata (file size, last modified, etc.)
- Inline editing of goals from the browser

## Your Implementation Approach

### Phase 1: Planning & Setup
1. **Read and understand** the entire specification first
2. **Ask clarifying questions** if anything is unclear or seems inconsistent
3. **Propose a task breakdown** using the TodoWrite tool based on the Implementation Plan section
4. **Set up dependencies** as the first concrete step

### Phase 2: Incremental Implementation
- Work through tasks in logical order (as outlined in the spec)
- **Use the TodoWrite tool** to track progress through all tasks
- Mark tasks as `in_progress` when starting, `completed` when done
- **Commit early and often** - create git commits for completed logical units
- **Test as you go** - don't defer testing until the end

### Phase 3: Integration & Polish
- Integrate with the main application
- Handle edge cases
- Verify all acceptance criteria from the spec
- Write or update documentation as needed

## Code Quality Standards

### Rust Best Practices
- Use idiomatic Rust: leverage the type system, avoid unwrap() in production code
- Follow existing code style and conventions in the codebase
- Use `anyhow::Result` for error handling consistently
- Add descriptive doc comments for public APIs
- Keep functions focused and composable

### Error Handling
- Provide helpful error messages to users
- Use `context()` from anyhow to add context to errors
- Handle edge cases gracefully (empty lists, missing files, etc.)

### Testing
- Write unit tests for state logic and pure functions
- Test edge cases and boundaries
- Use descriptive test names that explain what's being tested
- Keep tests focused and independent

### Comments & Documentation
- Add doc comments (`///`) for public structs, functions, and modules
- Use inline comments (`//`) sparingly - prefer self-documenting code
- Explain "why" not "what" in comments when needed

## Working with the Existing Codebase

### Integration Points (from spec)
The main integration point is in `src/main.rs` where `dialoguer::Select` is currently used. You'll replace this with your new TUI.

### Reuse Existing Types
- `DiscoveredGoal` from `src/config.rs` is your input type
- `find_all_goals()` already does the work of discovery
- `LoadedGoal` and `find_and_load_goal()` can help with loading prompt.yaml content

### Preserve Existing Behavior
- The output of the goal browser should be a goal name (String)
- The rest of the flow (`run_goal()` and beyond) should remain unchanged
- Other commands (`claw add`, `claw pass`, `claw <goal_name>`) should be unaffected

## Communication Guidelines

### Ask Questions When:
- Requirements are ambiguous or contradictory
- You need to make architectural decisions not covered in the spec
- You encounter unexpected issues or blockers
- You want to propose a better approach than specified

### Provide Updates:
- When starting a new major task
- When completing a task
- When you encounter issues or blockers
- When you need user input or decisions

### Keep It Concise:
- Focus on substance, not preamble
- Show code when relevant
- Explain your reasoning briefly when making decisions

## Getting Started

1. **Acknowledge** that you've read and understood this primer and the specification
2. **Ask any clarifying questions** about the requirements
3. **Create a todo list** based on the Implementation Plan in the spec
4. **Begin with Task 1** (Set Up Dependencies) and work incrementally

Remember: You are a collaborator, not just a code generator. Think critically, ask questions, and strive for quality. The goal is production-ready code, not just code that compiles.

Ready to begin!
