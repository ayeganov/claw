use clap::{Args, Parser, Subcommand};
use std::collections::HashMap;

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

    /// Arbitrary arguments for the prompt template, e.g., --lang=Python or --lang Python.
    /// All arguments after the goal name are collected here.
    #[arg(last = true)]
    pub template_args: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Subcommands {
    /// Add a new goal using an LLM-assisted workflow.
    Add {
        /// The name of the new goal to create.
        #[arg(required = true)]
        name: String,
    },
    /// Execute the underlying LLM CLI directly without any modifications.
    General,
}

/// Parses a vector of string arguments into a HashMap.
/// Supports formats: `--key=value` and `--key value`.
pub fn parse_template_args(args: &[String]) -> anyhow::Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--") {
            anyhow::bail!(
                "Invalid template argument: '{}'. All template arguments must be flags starting with '--'.",
                arg
            );
        }

        let key_part = &arg[2..]; // Remove the "--"
        if let Some((key, value)) = key_part.split_once('=') {
            // Handles --key=value
            map.insert(key.to_string(), value.to_string());
            i += 1;
        } else {
            // Handles --key value
            i += 1; // Move to the next item, which should be the value
            if i >= args.len() {
                anyhow::bail!("Argument '--{}' requires a value.", key_part);
            }
            let value = &args[i];
            if value.starts_with("--") {
                anyhow::bail!(
                    "Argument '--{}' requires a value, but found another flag '{}' instead.",
                    key_part,
                    value
                );
            }
            map.insert(key_part.to_string(), value.to_string());
            i += 1;
        }
    }
    Ok(map)
}
