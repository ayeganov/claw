<p align="center">
<img src="./assets/logo.png" alt="claw logo" width="512"/>
</p>

<h1 align="center">claw</h1>

<p align="center">
<strong>Your smart, context-aware AI coding partner on the command line.</strong>
</p>

<p align="center">
<a href="https://opensource.org/licenses/MIT">
<img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"/>
</a>
</p>

claw is a command-line utility that acts as an intelligent wrapper around your favorite Large Language Model (LLM) CLI (e.g., claude, gemini-cli). It transforms generic LLM sessions into powerful, goal-oriented workflows that are aware of your project's specific context, guidelines, and file structure.

Stop wasting time with repetitive setup prompts. With claw, you define a goal once and run it anywhere.

## Key Features
🎯 Goal-Oriented Sessions: Launch your LLM with a pre-defined purpose, like claw code-review or claw generate-tests.

📚 Cascading Configuration: Prioritizes repository-specific goals (./.claw/) over global user goals (~/.config/claw/), ensuring the most relevant context is always used.

⚙️ Dynamic Context Gathering: Execute shell scripts (git diff, ls -R) before a session starts to feed information directly into your prompt avoiding costly tokens.

📁 File Context Management: Pass arbitrary files and directories via --context to include their contents in your prompt, with smart filtering, size limits, and binary detection.

🤖 Agent-Assisted Goal Creation: Use the innovative claw add command to have an LLM agent interactively guide you through creating and refining new goals.

📜 Powerful Templating: Uses the tera engine to inject command-line arguments and script outputs into your prompts.

✅ Interactive Mode: Run claw with no arguments to get a clean, interactive menu of all available goals.

### Prerequisites
Before using claw, you must have an underlying LLM command-line tool installed and available in your system's PATH. `claw` is a wrapper and does not include an LLM itself.

Examples:

- Anthropic's Claude Console (claude)
- Google's Gemini CLI (gemini)

### Installation

#### From Releases (Recommended)
Download the latest release for your platform from the [releases page](https://github.com/ayeganov/claw/releases):

**Linux (Debian/Ubuntu)**
```bash
# Download the .deb package
wget https://github.com/ayeganov/claw/releases/latest/download/claw_VERSION_amd64.deb

# Install
sudo dpkg -i claw_VERSION_amd64.deb
```

**macOS**
```bash
# Download the .dmg file
# Open the DMG and drag claw.app to /Applications

# Add to PATH by creating a symlink
sudo ln -s /Applications/claw.app/Contents/MacOS/claw /usr/local/bin/claw
```

**Windows**
```bash
# Download the .msi installer and run it
# The installer will add claw to your PATH automatically
```

#### From crates.io
Once `claw` is published, you can install it directly from crates.io:

```bash
cargo install claw
```

#### From Source

```bash
git clone https://github.com/ayeganov/claw.git
cd claw
cargo build --release
# The binary will be in ./target/release/claw
```

The first time you run claw, it will automatically create a global configuration directory for you at ~/.config/claw/ with an example goal to get you started.

## Usage

### 1. Running a Goal
To run a goal, simply provide its name. Goal parameters are passed after `--` and are available in the prompt template.

```bash
# Run the 'code-review' goal
claw code-review

# Pass parameters to the prompt template
claw generate-component -- --name="UserProfile" --type="React"

# View goal-specific help and parameters
claw generate-component --explain
claw generate-component -e
```

### 2. Including File Context
Use the `--context` (or `-c`) flag to include files and directories in your prompt. This is perfect for code reviews, analysis, or any task that needs file contents.

```bash
# Include a single file
claw code-review --context src/main.rs

# Include multiple files
claw analyze --context file1.txt file2.txt config.json

# Include entire directories (recursive by default)
claw review --context ./src/ ./tests/

# Limit recursion depth
claw review --context ./src/ --recurse_depth 2

# Combine with goal parameters (note the -- separator)
claw review --context ./src/ -- --lang rust --scope authentication
```

**What happens:**
- Files are read and their contents are formatted as markdown
- Binary files are automatically skipped
- Respects `.gitignore` patterns
- Size limits and per-directory file limits are enforced
- The formatted context is appended to your prompt

**Configuration:** You can customize behavior in `claw.yaml`:
- `max_file_size_kb`: Maximum file size (default: 1024 KB)
- `max_files_per_directory`: Maximum files per directory (default: 50)
- `error_handling_mode`: How to handle errors - `strict`, `flexible`, or `ignore` (default: flexible)
- `excluded_directories`: Directories to skip (default: .git, node_modules, target, etc.)
- `excluded_extensions`: File extensions to skip (default: exe, bin, so, etc.)

### 3. Listing Goals
Use the `list` command to see all available goals and their parameters:

```bash
# List all goals
claw list

# List only local goals
claw list --local

# List only global goals
claw list --global
```

### 4. Interactive Mode
If you run claw without any arguments, it will display a menu of all available goals, indicating whether they are from your local project or your global config.

`claw`

### 5. Creating a New Goal (Agent-Assisted)
The `add` command launches an interactive LLM session to help you write a new prompt.yaml file.

```bash
# Start a session to create a new goal named 'pr-notes'
claw add pr-notes

# Force the new goal to be saved in the local project's .claw/ directory
claw add my-project-goal --local

# Force the new goal to be saved in the global ~/.config/claw/ directory
claw add my-global-goal --global
```

The agent will guide you through defining parameters if your goal needs them.

### 6. Direct Pass-Through
To open your underlying LLM directly without any modifications, use the `pass` command.

`claw pass`

# This is equivalent to just running 'claude' or 'gemini'

## Configuration
`claw` uses a simple configuration system based on YAML files.

### The `claw.yaml` File
This file configures which LLM claw should wrap and how context files are processed. It's looked for in ./.claw/ first, then ~/.config/claw/.

Example `~/.config/claw/claw.yaml`:

```yaml
# The executable name of the LLM CLI tool in your PATH.
llm_command: "claude"

# (Optional) The argument pattern for passing the prompt to the LLM.
# The "{{prompt}}" placeholder will be replaced with the final rendered prompt.
#
# Example for gemini-cli:
# prompt_arg_template: "-i {{prompt}}"

# Context Management 2.0 Configuration
# These settings control how claw processes files passed via --context parameter

# Maximum file size in KB that can be included as context (default: 1024 = 1 MB)
max_file_size_kb: 1024

# Maximum number of files per directory when scanning (default: 50)
max_files_per_directory: 50

# How to handle errors during context processing (default: flexible)
# Options:
#   strict: Fail immediately on any error
#   flexible: Collect all errors and prompt user for approval before proceeding
#   ignore: Log warnings but continue processing valid files
error_handling_mode: flexible

# Directories to exclude when scanning for context files
excluded_directories:
  - ".git"
  - "node_modules"
  - "target"
  - ".venv"
  - "__pycache__"

# File extensions to exclude when scanning for context files
excluded_extensions:
  - "exe"
  - "bin"
  - "so"
  - "dylib"
  - "dll"
  - "o"
  - "a"
```

### The `prompt.yaml` File
Each goal is defined by a prompt.yaml file located in a subdirectory of goals/.

Example `~/.config/claw/goals/pr-notes/prompt.yaml`:

```yaml
# A user-friendly name for display in lists.
name: "Pull Request Notes"

# A short, one-line description of the goal's purpose.
description: "Generates PR notes based on changes in the current branch."

# Optional: Define parameters that this goal accepts
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

# A map of shell commands to run before the prompt.
# The output of each command is injected into the main prompt.
context_scripts:
  branch_diff: "git diff main...HEAD"
  file_list: "git diff --name-only main...HEAD"

# The main prompt template sent to the LLM.
# It can use variables from context_scripts like {{ Context.branch_diff }}
# and from the command line like {{ Args.scope }}.
prompt: |
  You are an expert at writing release notes. Based on the following git diff,
  please generate concise PR notes for a pull request.

  The scope of this PR is: {{ Args.scope }}
  Format: {{ Args.format }}

  Changed Files:
  {{ Context.file_list }}

  --- GIT DIFF ---
  ```diff
  {{ Context.branch_diff }}

  --- END DIFF ---

  Please provide a title, a short summary, and a bulleted list of detailed changes.
```

**Using this goal:**
```bash
# View available parameters
claw pr-notes --explain

# Run with required parameter
claw pr-notes -- --scope authentication

# Run with optional parameters
claw pr-notes -- --scope api --format json
```


## License

This project is licensed under the MIT License.
