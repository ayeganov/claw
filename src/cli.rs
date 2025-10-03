use clap::{ArgGroup, Args, Parser, Subcommand};

/// A goal-driven, context-aware wrapper for Large Language Model (LLM) CLIs.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(arg_required_else_help = false)] // Allows running `claw` with no args for interactive mode
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Subcommands>,

    #[command(flatten)]
    pub run_args: RunArgs,
}

/// The run command arguments, flattened into the main CLI struct.
/// This allows `claw [goal_name]` to work without a `run` subcommand.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Name of the goal to run.
    #[arg(name = "GOAL")]
    pub goal_name: Option<String>,

    /// Files or directories to include as context.
    #[arg(short = 'c', long = "context", num_args = 0..)]
    pub context: Vec<std::path::PathBuf>,

    /// Maximum recursion depth when scanning directories (default: unlimited).
    #[arg(short = 'd', long = "recurse_depth")]
    pub recurse_depth: Option<usize>,

    /// Show detailed information about the goal's parameters.
    #[arg(short = 'e', long = "explain")]
    pub explain: bool,

    /// Arbitrary arguments for the prompt template, e.g., --lang=Python or --lang Python.
    /// All arguments after the goal name are collected here.
    #[arg(last = true)]
    pub template_args: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Subcommands {
    #[command(group(ArgGroup::new("location").args(["local", "global"])))]
    Add {
        /// The name of the new goal to create.
        #[arg(required = true)]
        name: String,

        /// Force creation of the goal in the local .claw/ directory.
        #[arg(long)]
        local: bool,

        /// Force creation of the goal in the global ~/.config/claw directory.
        #[arg(long)]
        global: bool,
    },
    /// List all available goals with their descriptions and parameters.
    #[command(group(ArgGroup::new("filter").args(["local", "global"])))]
    List {
        /// Show only local goals from .claw/ directory.
        #[arg(long)]
        local: bool,

        /// Show only global goals from ~/.config/claw directory.
        #[arg(long)]
        global: bool,
    },
    /// Execute the underlying LLM CLI directly without any modifications.
    Pass,
}
