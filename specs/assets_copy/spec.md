# Specification: Simplified Asset Directory Copying

## Overview

### Problem Statement
The current `ensure_global_config_exists()` function in `src/config.rs` manually copies specific files and directories from the bundled assets directory to the user's global config directory (`~/.config/claw/`). This approach:
- Requires hardcoded references to specific files (`claw.yaml`) and directories (`goals/`)
- Makes maintenance difficult when adding new assets
- Risks missing files if the assets structure changes
- Contains repetitive error-handling code for each copy operation

### Goals
1. **Simplify the setup process** by copying the entire assets directory in one operation
2. **Improve maintainability** by eliminating hardcoded file and folder references
3. **Future-proof the codebase** by ensuring all future assets are automatically included without code changes
4. **Preserve user experience** with clear, friendly messaging during first-time setup

### Target Users
End users (developers) installing and using claw for the first time.

### Success Criteria
- ✅ Fresh installation: All contents of the assets directory are recursively copied to `~/.config/claw/`
- ✅ Existing installation: No changes are made if the config directory already contains files
- ✅ Directory structure from assets is preserved in the target location
- ✅ User sees clear, informative messages during the setup process
- ✅ No breaking changes to existing user workflows

---

## Requirements

### Functional Requirements

#### FR-1: Empty Directory Detection
**Priority**: High

The function must check if the global config directory (`~/.config/claw/`) is empty before attempting to copy assets.

#### FR-2: Recursive Directory Copy
**Priority**: High

When the config directory is empty, the function must recursively copy all contents of the assets directory, preserving the directory structure.

#### FR-3: Non-Overwrite Behavior
**Priority**: High

If the config directory is not empty (i.e., contains any files or subdirectories), the function must not perform any copy operations.

#### FR-4: User Messaging
**Priority**: Medium

The function must provide clear, friendly messages to the user explaining:
- That this is a first-time setup
- Where the config directory is being created
- What assets are being copied
- How to get started with an example command

#### FR-5: Error Handling
**Priority**: High

The function must provide clear error messages with context when:
- The config directory cannot be created
- The assets directory cannot be found
- The copy operation fails

### Non-Functional Requirements

#### NFR-1: Code Simplicity
The implementation should be significantly simpler than the current approach, with fewer lines of code and less repetitive logic.

#### NFR-2: Consistency with Existing Code Style
The implementation must follow the existing patterns in `src/config.rs`:
- Use `anyhow::Result` for error handling
- Use `.context()` for adding error context
- Follow Rust idioms and conventions

#### NFR-3: No Performance Concerns
The asset directory is small and controlled, so performance optimization is not required.

### Dependencies and Prerequisites

#### External Dependencies
- **fs_extra**: Rust library providing advanced file system operations, including recursive directory copying

#### Internal Dependencies
- `find_assets_dir()`: Existing function to locate the bundled assets directory
- `BaseDirs::new()`: From the `directories` crate, used to find the user's config directory

---

## Architecture & Design

### High-Level Design

The refactored `ensure_global_config_exists()` function will follow this flow:

```
1. Resolve user's config directory (~/.config/claw/)
   ├─ If resolution fails → Return (no error, just skip setup)
   └─ If resolved → Continue

2. Check if config directory exists
   ├─ If exists → Check if empty
   │  ├─ If not empty → Return (existing install, do nothing)
   │  └─ If empty → Continue to step 3
   └─ If doesn't exist → Create it and continue to step 3

3. Display welcome message to user

4. Locate assets directory using find_assets_dir()
   └─ If not found → Return error with context

5. Copy entire assets directory contents to config directory
   └─ If copy fails → Return error with context

6. Display success message with example command
```

### Key Components

#### Component 1: Directory Empty Check
**Responsibility**: Determine if a directory is empty (contains no files or subdirectories)

**Interface**:
```rust
fn is_directory_empty(path: &Path) -> Result<bool>
```

**Behavior**:
- Returns `Ok(true)` if the directory exists and contains no entries
- Returns `Ok(true)` if the directory doesn't exist
- Returns `Ok(false)` if the directory contains any files or subdirectories
- Returns `Err` if the directory cannot be read

#### Component 2: Recursive Directory Copy
**Responsibility**: Copy all contents from source directory to destination directory

**Implementation**: Use `fs_extra::dir::copy()` with appropriate options

**Options Configuration**:
```rust
CopyOptions {
    overwrite: false,        // Don't overwrite existing files
    skip_exist: true,        // Skip files that already exist
    copy_inside: true,       // Copy contents INTO the target directory
    content_only: true,      // Copy only the contents, not the directory itself
    ..Default::default()
}
```

#### Component 3: Refactored Main Function
**Responsibility**: Orchestrate the setup process

**Signature**:
```rust
pub fn ensure_global_config_exists() -> Result<()>
```

**Key Changes from Current Implementation**:
- Remove individual file/directory copy operations
- Remove manual iteration over goals directory
- Replace with single recursive copy operation
- Add empty directory check before copying

### Data Structures

No new data structures are required. The function will use existing types:
- `PathBuf`: For file system paths
- `anyhow::Result`: For error handling
- `fs_extra::dir::CopyOptions`: For configuring the copy operation

### Integration Points

#### Existing Code Integration
The function is called during claw initialization, and this behavior remains unchanged. The function signature and return type remain the same, ensuring compatibility with calling code.

#### File System Integration
- **Input**: Bundled assets directory (location resolved by `find_assets_dir()`)
- **Output**: User's global config directory (`~/.config/claw/`)

---

## Implementation Plan

### Task 1: Add fs_extra Dependency
**Description**: Add the `fs_extra` crate to `Cargo.toml`

**Acceptance Criteria**:
- `fs_extra` is added to `[dependencies]` in `Cargo.toml`
- Version is specified (e.g., `fs_extra = "1.3"`)
- Project compiles successfully with the new dependency

**Estimated Effort**: Trivial (5 minutes)

### Task 2: Implement is_directory_empty Helper Function
**Description**: Create a helper function to check if a directory is empty

**Implementation Details**:
```rust
fn is_directory_empty(path: &Path) -> Result<bool> {
    // If directory doesn't exist, consider it "empty"
    if !path.exists() {
        return Ok(true);
    }

    // Read directory and check if it has any entries
    let mut entries = fs::read_dir(path)
        .context("Failed to read directory")?;

    // If no entries, it's empty
    Ok(entries.next().is_none())
}
```

**Acceptance Criteria**:
- Function returns `Ok(true)` for non-existent directories
- Function returns `Ok(true)` for empty directories
- Function returns `Ok(false)` for directories with content
- Function returns `Err` with context for unreadable directories

**Estimated Effort**: Small (15 minutes)

### Task 3: Refactor ensure_global_config_exists
**Description**: Replace manual file copying with recursive directory copy using `fs_extra`

**Implementation Details**:
1. Remove all individual file/directory copy code
2. Add empty directory check after creating config_dir
3. Use `fs_extra::dir::copy()` to copy entire assets directory
4. Update welcome message if needed for clarity

**Key Code Changes**:
```rust
// After creating config_dir, before any copying:
if !is_directory_empty(&config_dir)? {
    // Directory already has content, don't overwrite
    return Ok(());
}

// Display welcome message (existing or slightly updated)

// Find assets directory (existing)
let assets_dir = find_assets_dir()
    .context("Failed to locate assets for first-time setup")?;

// Copy entire assets directory contents
let mut options = fs_extra::dir::CopyOptions::new();
options.overwrite = false;
options.skip_exist = true;
options.copy_inside = true;
options.content_only = true;

fs_extra::dir::copy(&assets_dir, &config_dir, &options)
    .context("Failed to copy assets to config directory")?;

// Display success message (existing or slightly updated)
```

**Acceptance Criteria**:
- Function checks if config directory is empty before copying
- All assets are copied recursively in a single operation
- No hardcoded file or directory names remain
- Error handling uses `.context()` for clarity
- User sees appropriate messages during setup

**Estimated Effort**: Medium (30-45 minutes)

### Task 4: Update Welcome/Success Messages (Optional)
**Description**: Review and optionally update user-facing messages for clarity

**Current Messages**:
- "I've created a `claw.yaml` file there to get you started."
- "I've also added some example goals."

**Potential Updates**:
- "I've copied the default configuration and example goals to get you started."
- Keep existing messages if they're clear enough

**Acceptance Criteria**:
- Messages accurately reflect what the function does
- Tone remains friendly and helpful
- Example command is still shown

**Estimated Effort**: Trivial (5 minutes)

### Task Sequencing

```
Task 1 (Add dependency)
    ↓
Task 2 (Implement helper) ← Can run in parallel with Task 4
    ↓
Task 3 (Refactor main function)
    ↓
Task 4 (Update messages) ← Optional, can be done last
```

**Total Estimated Effort**: 1-1.5 hours

---

## Testing Strategy

### Test Scenarios

#### Scenario 1: Fresh Installation (Happy Path)
**Setup**:
- Remove `~/.config/claw/` directory if it exists
- Ensure assets directory exists with known contents

**Execution**:
- Run `ensure_global_config_exists()`

**Expected Results**:
- Config directory is created at `~/.config/claw/`
- All files from assets are present in config directory
- Directory structure is preserved
- Welcome message is displayed
- Function returns `Ok(())`

**Verification**:
```rust
assert!(config_dir.exists());
assert!(config_dir.join("claw.yaml").exists());
assert!(config_dir.join("goals").is_dir());
assert!(config_dir.join("goals/example").is_dir());
assert!(config_dir.join("goals/example/prompt.yaml").exists());
```

#### Scenario 2: Existing Installation (Non-Empty Directory)
**Setup**:
- Create `~/.config/claw/` directory
- Add at least one file or subdirectory to it

**Execution**:
- Run `ensure_global_config_exists()`

**Expected Results**:
- No files are copied
- Existing files remain unchanged
- No welcome message is displayed
- Function returns `Ok(())`

**Verification**:
```rust
// Count files before and after
let files_before = count_files(&config_dir);
ensure_global_config_exists()?;
let files_after = count_files(&config_dir);
assert_eq!(files_before, files_after);
```

#### Scenario 3: Empty Existing Directory
**Setup**:
- Create `~/.config/claw/` directory (empty)

**Execution**:
- Run `ensure_global_config_exists()`

**Expected Results**:
- All assets are copied (same as Scenario 1)
- Welcome message is displayed
- Function returns `Ok(())`

#### Scenario 4: Assets Directory Not Found
**Setup**:
- Mock or manipulate environment so `find_assets_dir()` fails

**Execution**:
- Run `ensure_global_config_exists()`

**Expected Results**:
- Function returns `Err` with context message about missing assets
- Config directory may be created but remains empty

**Verification**:
```rust
let result = ensure_global_config_exists();
assert!(result.is_err());
assert!(result.unwrap_err().to_string().contains("assets"));
```

#### Scenario 5: Permission Errors
**Setup**:
- Make config directory read-only or otherwise restrict permissions

**Execution**:
- Run `ensure_global_config_exists()`

**Expected Results**:
- Function returns `Err` with context about the failure
- Error message is clear and actionable

### Acceptance Criteria

#### Must Have:
- ✅ All assets are copied on fresh installation
- ✅ No files are copied if config directory is not empty
- ✅ Directory structure is preserved
- ✅ Clear error messages on failure
- ✅ No hardcoded file/directory names in the code

#### Should Have:
- ✅ Friendly user messages during setup
- ✅ Graceful handling of edge cases (permissions, missing assets)

#### Nice to Have:
- Unit tests for `is_directory_empty()` helper
- Integration test covering the full setup flow

### Manual Testing Checklist

Before merging:
- [ ] Test fresh installation on Linux
- [ ] Test fresh installation on macOS
- [ ] Test fresh installation on Windows
- [ ] Test with existing config directory (non-empty)
- [ ] Test with empty config directory
- [ ] Verify all assets are present after setup
- [ ] Verify no errors in console output
- [ ] Verify example goal works: `claw example -- --topic="test"`

---

## Open Questions

None at this time. All requirements and constraints have been clarified.
