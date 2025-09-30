use anyhow::Context as AnyhowContext;
use anyhow::Result;
use directories::BaseDirs;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct ClawConfig {
    /// The executable name of the LLM command-line tool.
    pub llm_command: String,

    /// The argument template for passing the prompt to the LLM.
    /// Uses "{{prompt}}" as a placeholder for the rendered prompt.
    #[serde(default = "default_prompt_arg_template")]
    pub prompt_arg_template: String,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalSource {
    Local,
    Global,
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
    let path = base_dir.join("goals").join(goal_name).join("prompt.yaml");

    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    let config: PromptConfig = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?;

    Ok(Some(config))
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

    // Priority 1: Search in the local repository config (`.claw/`)
    if let Some(local_path) = &paths.local {
        if let Some(config) = load_goal_config(local_path, goal_name)? {
            let directory = local_path.join("goals").join(goal_name);
            return Ok(LoadedGoal { config, directory });
        }
    }

    // Priority 2: Search in the global user config (`~/.config/claw/`)
    if let Some(global_path) = &paths.global {
        if let Some(config) = load_goal_config(global_path, goal_name)? {
            let directory = global_path.join("goals").join(goal_name);
            return Ok(LoadedGoal { config, directory });
        }
    }

    // If we reach here, the goal was not found in either location.
    anyhow::bail!(
        "Goal '{}' not found in local or global configuration.",
        goal_name
    );
}

/// Finds and loads the `claw.yaml` configuration, applying the cascade and defaults.
///
/// 1. Searches for `claw.yaml` in the local `.claw/` directory.
/// 2. If not found, falls back to the global `~/.config/claw/` directory.
/// 3. If no file is found in either location, it returns `ClawConfig::default()`.
/// This function always returns a valid configuration.
pub fn find_and_load_claw_config() -> Result<ClawConfig> {
    let paths = ConfigPaths::new()?;

    // Priority 1: Search in the local repository config (`.claw/`)
    if let Some(local_path) = &paths.local {
        if let Some(config) = load_claw_config_from_dir(local_path)? {
            return Ok(config);
        }
    }

    // Priority 2: Search in the global user config (`~/.config/claw/`)
    if let Some(global_path) = &paths.global {
        if let Some(config) = load_claw_config_from_dir(global_path)? {
            return Ok(config);
        }
    }

    // Priority 3: Fall back to the compiled-in default configuration.
    Ok(ClawConfig::default())
}

/// Helper to attempt loading a `claw.yaml` from a single directory.
fn load_claw_config_from_dir(base_dir: &Path) -> Result<Option<ClawConfig>> {
    let path = base_dir.join("claw.yaml");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    let config: ClawConfig = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?;

    Ok(Some(config))
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

/// Scans local and global directories to find all available goals.
/// Local goals with the same name as global goals will override them.
pub fn find_all_goals() -> Result<Vec<DiscoveredGoal>> {
    let paths = ConfigPaths::new()?;
    let mut discovered_goals = Vec::new();
    let mut seen_names = HashSet::new();

    // Priority 1: Find all local goals
    if let Some(local_path) = &paths.local {
        let local_goals_dir = local_path.join("goals");
        if local_goals_dir.is_dir() {
            for entry in fs::read_dir(local_goals_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(config) = load_goal_config(local_path, &name)? {
                        seen_names.insert(name.clone());
                        discovered_goals.push(DiscoveredGoal {
                            name,
                            source: GoalSource::Local,
                            config,
                        });
                    }
                }
            }
        }
    }

    // Priority 2: Find all global goals that haven't been seen locally
    if let Some(global_path) = &paths.global {
        let global_goals_dir = global_path.join("goals");
        if global_goals_dir.is_dir() {
            for entry in fs::read_dir(global_goals_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !seen_names.contains(&name) {
                        if let Some(config) = load_goal_config(global_path, &name)? {
                            discovered_goals.push(DiscoveredGoal {
                                name,
                                source: GoalSource::Global,
                                config,
                            });
                        }
                    }
                }
            }
        }
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
    // - `../share/claw/assets`: For common Linux installations (`/usr/bin/claw`)
    // - `./assets`: For when assets are next to the exe (Windows .zip installs)
    let potential_paths = [
        PathBuf::from("../assets"),
        PathBuf::from("../share/claw/assets"),
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
            let example_goal_dest_dir = goals_dir.join("example");
            fs::create_dir_all(&example_goal_dest_dir)
                .context("Failed to create example goal directory")?;
            let prompt_path = example_goal_dest_dir.join("prompt.yaml");
            let source_prompt_path = assets_dir.join("goals").join("example").join("prompt.yaml");
            fs::copy(&source_prompt_path, &prompt_path).with_context(|| {
                format!(
                    "Failed to copy example goal from {} to {}",
                    source_prompt_path.display(),
                    prompt_path.display()
                )
            })?;

            println!("I've also added an example goal. Try it out by running:");
            println!("claw example --topic=\"the history of the Rust programming language\"");
            println!("--------------------------------------------------------------------");
        }
    }
    Ok(())
}
