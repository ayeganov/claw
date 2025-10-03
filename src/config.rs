use anyhow::Context as AnyhowContext;
use anyhow::Result;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Helper functions for constructing standard configuration paths.
mod paths {
    use std::path::{Path, PathBuf};

    pub fn goal_dir(base_dir: &Path, goal_name: &str) -> PathBuf {
        base_dir.join("goals").join(goal_name)
    }

    pub fn goal_prompt(base_dir: &Path, goal_name: &str) -> PathBuf {
        goal_dir(base_dir, goal_name).join("prompt.yaml")
    }

    pub fn claw_config(base_dir: &Path) -> PathBuf {
        base_dir.join("claw.yaml")
    }
}

/// Generic function to load and parse a YAML config file.
///
/// Returns `Ok(Some(config))` if the file exists and is parsed successfully.
/// Returns `Ok(None)` if the file does not exist.
/// Returns `Err` if the file exists but cannot be read or parsed.
fn load_yaml_config<T>(path: &Path) -> Result<Option<T>>
where
    T: for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let config: T = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    Ok(Some(config))
}

/// Generic cascading configuration loader.
///
/// Searches for a configuration in priority order:
/// 1. Local repository config
/// 2. Global user config
/// 3. Default value (if provided)
///
/// The `loader_fn` is called with the base directory to attempt loading the config.
fn cascade_load_config<T, F>(
    paths: &ConfigPaths,
    loader_fn: F,
    default: Option<T>,
) -> Result<T>
where
    F: Fn(&Path) -> Result<Option<T>>,
{
    // Priority 1: Local repository config
    if let Some(local_path) = &paths.local {
        if let Some(config) = loader_fn(local_path)? {
            return Ok(config);
        }
    }

    // Priority 2: Global user config
    if let Some(global_path) = &paths.global {
        if let Some(config) = loader_fn(global_path)? {
            return Ok(config);
        }
    }

    // Priority 3: Default or error
    default.ok_or_else(|| anyhow::anyhow!("Configuration not found in local or global paths"))
}

/// Defines how errors during context processing should be handled.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorHandlingMode {
    /// Fail immediately on any error.
    Strict,
    /// Collect all errors and prompt user for approval before proceeding.
    Flexible,
    /// Log warnings but continue processing valid files.
    Ignore,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClawConfig {
    /// The executable name of the LLM command-line tool.
    pub llm_command: String,

    /// The argument template for passing the prompt to the LLM.
    /// Uses "{{prompt}}" as a placeholder for the rendered prompt.
    #[serde(default = "default_prompt_arg_template")]
    pub prompt_arg_template: String,

    // Context Management 2.0 fields
    /// Maximum file size in KB that can be included as context.
    #[serde(default)]
    pub max_file_size_kb: Option<u64>,

    /// Maximum number of files per directory when scanning.
    #[serde(default)]
    pub max_files_per_directory: Option<usize>,

    /// How to handle errors during context processing: "strict", "flexible", or "ignore".
    #[serde(default)]
    pub error_handling_mode: Option<ErrorHandlingMode>,

    /// Directories to exclude when scanning for context files.
    #[serde(default)]
    pub excluded_directories: Option<Vec<String>>,

    /// File extensions to exclude when scanning for context files.
    #[serde(default)]
    pub excluded_extensions: Option<Vec<String>>,
}

/// Provides the default value for `prompt_arg_template` during deserialization.
fn default_prompt_arg_template() -> String {
    "{{prompt}}".to_string()
}

/// Provides a complete, fallback configuration if no `claw.yaml` is found.
impl Default for ClawConfig {
    fn default() -> Self {
        Self {
            // We default to "claude" as it's a common tool with a simple invocation.
            llm_command: "claude".to_string(),
            prompt_arg_template: default_prompt_arg_template(),
            // Context Management 2.0 defaults
            max_file_size_kb: Some(1024), // 1 MB
            max_files_per_directory: Some(50),
            error_handling_mode: Some(ErrorHandlingMode::Flexible),
            excluded_directories: Some(vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".venv".to_string(),
                "__pycache__".to_string(),
            ]),
            excluded_extensions: Some(vec![
                "exe".to_string(),
                "bin".to_string(),
                "so".to_string(),
                "dylib".to_string(),
                "dll".to_string(),
                "o".to_string(),
                "a".to_string(),
            ]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalSource {
    Local,
    Global,
}

/// Represents the type of a goal parameter.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
}

/// Represents a single parameter definition for a goal.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalParameter {
    /// The name of the parameter (e.g., "scope", "format").
    pub name: String,

    /// Human-readable description of what this parameter does.
    pub description: String,

    /// Whether this parameter must be provided by the user.
    pub required: bool,

    /// Optional type hint for the parameter (primarily for documentation).
    #[serde(default)]
    #[serde(rename = "type")]
    pub param_type: Option<ParameterType>,

    /// Optional default value for the parameter (only valid if required is false).
    #[serde(default)]
    pub default: Option<String>,
}

/// Represents the structure of a `prompt.yaml` file.
///
/// This struct is derived with `serde::Deserialize` to allow for automatic
/// parsing from a YAML string into a typed Rust object.
#[derive(Debug, Clone, Deserialize)]
pub struct PromptConfig {
    /// A user-friendly name for the goal, e.g., "Staged Git Changes Code Review".
    pub name: String,

    /// An optional one-line description of the goal's purpose.
    pub description: Option<String>,

    /// Optional list of parameters that this goal accepts.
    /// If not specified, the goal accepts arbitrary parameters.
    #[serde(default)]
    pub parameters: Vec<GoalParameter>,

    /// A map of script names to the shell commands to be executed.
    /// The key is the name used in the template (e.g., `staged_diff`),
    /// and the value is the command to run (e.g., "git diff --staged").
    /// `#[serde(default)]` ensures that if `context_scripts` is missing from
    /// the YAML, this field will be an empty HashMap instead of causing an error.
    #[serde(default)]
    pub context_scripts: HashMap<String, String>,

    /// The Tera template string for the prompt.
    pub prompt: String,
}

/// Holds the resolved paths for local (repository) and global (user) configurations.
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    /// The path to the repository-specific `.claw/` directory, if found.
    pub local: Option<PathBuf>,
    /// The path to the global `~/.config/claw/` directory, if it exists.
    pub global: Option<PathBuf>,
}

impl ConfigPaths {
    /// Finds and returns the local and global configuration paths.
    pub fn new() -> Result<Self> {
        Ok(Self {
            local: find_local_config_dir()?,
            global: find_global_config_dir(),
        })
    }
}

/// Searches upwards from the current directory for a `.claw` directory.
fn find_local_config_dir() -> Result<Option<PathBuf>> {
    let current_dir = env::current_dir()?;
    for ancestor in current_dir.ancestors() {
        let claw_dir = ancestor.join(".claw");
        if claw_dir.is_dir() {
            return Ok(Some(claw_dir));
        }
    }
    Ok(None)
}

/// Returns the path to the global config directory, `~/.config/claw/`.
fn find_global_config_dir() -> Option<PathBuf> {
    if let Some(base_dirs) = BaseDirs::new() {
        let config_dir = base_dirs.config_dir().join("claw");
        if config_dir.exists() {
            return Some(config_dir);
        }
    }
    None
}

/// Loads and parses a `prompt.yaml` file for a specific goal from a base directory.
///
/// It returns `Ok(Some(config))` if the goal is found and parsed successfully.
/// It returns `Ok(None)` if the `prompt.yaml` file does not exist.
/// It returns an `Err` if the file exists but cannot be read or parsed.
pub fn load_goal_config(base_dir: &Path, goal_name: &str) -> Result<Option<PromptConfig>> {
    let path = paths::goal_prompt(base_dir, goal_name);
    load_yaml_config(&path)
}

/// Represents a successfully loaded goal configuration, including its content
/// and the path to its directory. The path is needed to resolve relative
/// paths for features like file inclusion in Tera templates.
#[derive(Debug, Clone)]
pub struct LoadedGoal {
    pub config: PromptConfig,
    pub directory: PathBuf,
}

/// Implements the configuration cascade to find and load a specific goal.
///
/// 1. Searches for the goal in the local `.claw/` directory.
/// 2. If not found, falls back to the global `~/.config/claw/` directory.
/// 3. Returns an error if the goal is not found in either location.
pub fn find_and_load_goal(goal_name: &str) -> Result<LoadedGoal> {
    let paths = ConfigPaths::new()?;
    let goal_name = goal_name.to_string();

    cascade_load_config(
        &paths,
        |base_dir| {
            if let Some(config) = load_goal_config(base_dir, &goal_name)? {
                let directory = paths::goal_dir(base_dir, &goal_name);
                Ok(Some(LoadedGoal { config, directory }))
            } else {
                Ok(None)
            }
        },
        None,
    )
    .with_context(|| format!("Goal '{}' not found in local or global configuration", goal_name))
}

/// Finds and loads the `claw.yaml` configuration, applying the cascade and defaults.
///
/// 1. Searches for `claw.yaml` in the local `.claw/` directory.
/// 2. If not found, falls back to the global `~/.config/claw/` directory.
/// 3. If no file is found in either location, it returns `ClawConfig::default()`.
/// This function always returns a valid configuration.
pub fn find_and_load_claw_config() -> Result<ClawConfig> {
    let paths = ConfigPaths::new()?;
    cascade_load_config(&paths, load_claw_config_from_dir, Some(ClawConfig::default()))
}

/// Helper to attempt loading a `claw.yaml` from a single directory.
fn load_claw_config_from_dir(base_dir: &Path) -> Result<Option<ClawConfig>> {
    let path = paths::claw_config(base_dir);
    load_yaml_config(&path)
}

impl fmt::Display for GoalSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoalSource::Local => write!(f, "local"),
            GoalSource::Global => write!(f, "global"),
        }
    }
}

/// Represents a goal found in either the local or global config.
#[derive(Debug, Clone)]
pub struct DiscoveredGoal {
    pub name: String,
    pub source: GoalSource,
    pub config: PromptConfig,
}

/// Scans a goals directory and returns discovered goals with the given source.
fn scan_goals_dir(base_dir: &Path, source: GoalSource) -> Result<Vec<DiscoveredGoal>> {
    let mut discovered = Vec::new();
    let goals_dir = base_dir.join("goals");

    if !goals_dir.is_dir() {
        return Ok(discovered);
    }

    for entry in fs::read_dir(goals_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(config) = load_goal_config(base_dir, &name)? {
                discovered.push(DiscoveredGoal {
                    name,
                    source,
                    config,
                });
            }
        }
    }

    Ok(discovered)
}

/// Scans local and global directories to find all available goals.
/// Local goals with the same name as global goals will override them.
pub fn find_all_goals() -> Result<Vec<DiscoveredGoal>> {
    let paths = ConfigPaths::new()?;
    let mut discovered_goals = Vec::new();

    // Priority 1: Find all local goals
    if let Some(local_path) = &paths.local {
        discovered_goals.extend(scan_goals_dir(local_path, GoalSource::Local)?);
    }

    // Priority 2: Find all global goals
    if let Some(global_path) = &paths.global {
        discovered_goals.extend(scan_goals_dir(global_path, GoalSource::Global)?);
    }

    // Sort goals alphabetically by name for a clean display
    discovered_goals.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(discovered_goals)
}

fn find_assets_dir() -> Result<PathBuf> {
    let exe_path = env::current_exe().context("Failed to get current executable path")?;
    let exe_dir = exe_path
        .parent()
        .context("Failed to get parent directory of executable")?;

    // List of potential relative paths to the assets directory.
    // - `../assets`: For development (`target/debug/claw`)
    // - `../lib/claw/assets`: For cargo-packager deb packages (`/usr/bin/claw`)
    // - `../share/claw/assets`: For common Linux installations (`/usr/bin/claw`)
    // - `../Resources/assets`: For macOS app bundles (`/Applications/claw.app/Contents/MacOS/claw`)
    // - `./assets`: For when assets are next to the exe (Windows .zip installs)
    let potential_paths = [
        PathBuf::from("../assets"),
        PathBuf::from("../lib/claw/assets"),
        PathBuf::from("../share/claw/assets"),
        PathBuf::from("../Resources/assets"),
        PathBuf::from("assets"),
    ];

    for path in &potential_paths {
        let assets_dir = exe_dir.join(path);
        if assets_dir.is_dir() {
            return Ok(assets_dir);
        }
    }

    anyhow::bail!(
        "Could not find the bundled assets directory. Searched in paths relative to {}",
        exe_dir.display()
    );
}

pub fn ensure_global_config_exists() -> Result<()> {
    if let Some(base_dirs) = BaseDirs::new() {
        let config_dir = base_dirs.config_dir().join("claw");
        // If the main config directory doesn't exist, we assume it's a first run.
        if !config_dir.exists() {
            println!(
                "
Welcome to claw! 🐾
This looks like your first time. I'm creating a global config directory for you at:
{}

I've created a `claw.yaml` file there to get you started.
You can edit it to change the underlying LLM command.
",
                config_dir.display()
            );

            fs::create_dir_all(&config_dir).with_context(|| {
                format!(
                    "Failed to create claw config directory at {}",
                    config_dir.display()
                )
            })?;

            let assets_dir =
                find_assets_dir().context("Failed to locate assets for first-time setup")?;

            let config_path = config_dir.join("claw.yaml");

            let source_config_path = assets_dir.join("claw.yaml");
            fs::copy(&source_config_path, &config_path).with_context(|| {
                format!(
                    "Failed to copy default config from {} to {}",
                    source_config_path.display(),
                    config_path.display()
                )
            })?;

            let goals_dir = config_dir.join("goals");
            fs::create_dir_all(&goals_dir).context("Failed to create goals directory")?;

            // Copy all goals from assets
            let assets_goals_dir = assets_dir.join("goals");
            if assets_goals_dir.is_dir() {
                for entry in fs::read_dir(&assets_goals_dir)
                    .context("Failed to read assets goals directory")?
                {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() {
                        let goal_name = entry.file_name();
                        let dest_goal_dir = goals_dir.join(&goal_name);
                        fs::create_dir_all(&dest_goal_dir).with_context(|| {
                            format!("Failed to create goal directory for {:?}", goal_name)
                        })?;

                        let source_prompt = entry.path().join("prompt.yaml");
                        let dest_prompt = dest_goal_dir.join("prompt.yaml");
                        fs::copy(&source_prompt, &dest_prompt).with_context(|| {
                            format!(
                                "Failed to copy goal {:?} from {} to {}",
                                goal_name,
                                source_prompt.display(),
                                dest_prompt.display()
                            )
                        })?;
                    }
                }
            }

            println!("I've also added some example goals. Try one out by running:");
            println!("claw example -- --topic=\"the history of the Rust programming language\"");
            println!("--------------------------------------------------------------------");
        }
    }
    Ok(())
}
