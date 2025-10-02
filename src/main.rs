mod cli;
mod commands;
mod config;
mod context;
mod goal_browser;
mod runner;

use anyhow::{Context as AnyhowContext, Result};
use clap::Parser;
use cli::{Cli, Subcommands};
use std::collections::HashMap;
use tera::{Context, Tera};

fn main() -> Result<()> {
    config::ensure_global_config_exists()?;

    // Load the main claw configuration (cascading)
    let claw_config = config::find_and_load_claw_config()?;

    let cli = Cli::parse();

    match cli.command {
        Some(Subcommands::Add {
            name,
            local,
            global,
        }) => {
            commands::add::handle_add_command(&name, local, global, &claw_config)?;
        }
        Some(Subcommands::Pass) => {
            runner::run_pass_through(&claw_config)?;
        }
        None => {
            if let Some(goal_name) = cli.run_args.goal_name {
                run_goal(
                    &goal_name,
                    &claw_config,
                    &cli.run_args.template_args,
                    &cli.run_args.context,
                    cli.run_args.recurse_depth,
                )?;
            } else {
                // No goal was provided, so enter interactive mode.
                let goals = config::find_all_goals()?;
                if goals.is_empty() {
                    anyhow::bail!("No goals found. Add a goal using `claw add <goal_name>`.");
                }

                // Use the new goal browser TUI
                let selected_goal_name = goal_browser::run_goal_browser(goals)?;

                run_goal(&selected_goal_name, &claw_config, &Vec::new(), &Vec::new(), None)?;
            }
        }
    }

    Ok(())
}

fn run_goal(
    goal_name: &str,
    claw_config: &config::ClawConfig,
    template_args: &[String],
    context_paths: &[std::path::PathBuf],
    recurse_depth: Option<usize>,
) -> Result<()> {
    let goal = config::find_and_load_goal(goal_name)?;
    let template_args = cli::parse_template_args(template_args)?;

    // Create a Tera context with Args for rendering context scripts
    let mut context = Context::new();
    context.insert("Args", &template_args);

    // Render the context scripts through Tera to substitute Args variables
    let mut tera = Tera::default();
    let mut rendered_scripts = HashMap::new();
    for (name, script_template) in &goal.config.context_scripts {
        tera.add_raw_template(name, script_template)
            .with_context(|| format!("Failed to add context script template '{}'", name))?;
        let rendered_script = tera
            .render(name, &context)
            .map_err(|e| anyhow::anyhow!("Failed to render context script '{}': {}", name, e))?;
        rendered_scripts.insert(name.clone(), rendered_script);
    }

    // Execute the rendered context scripts
    let script_outputs = runner::execute_context_scripts(&rendered_scripts)?;
    context.insert("Context", &script_outputs);

    // Now render the main prompt with both Args and Context
    let mut tera = Tera::new(&format!("{}/**/*", goal.directory.display()))
        .context("Failed to create Tera instance")?;
    tera.add_raw_template("prompt", &goal.config.prompt)
        .context("Failed to add raw template")?;
    let mut rendered_prompt = tera
        .render("prompt", &context)
        .map_err(|e| anyhow::anyhow!("Failed to render prompt for goal '{}': {}", goal_name, e))?;

    // Process file context if --context parameter was provided
    if !context_paths.is_empty() {
        let context_config = context::ContextConfig {
            paths: context_paths.to_vec(),
            recurse_depth,
            max_file_size_kb: claw_config.max_file_size_kb.unwrap_or(1024),
            max_files_per_directory: claw_config.max_files_per_directory.unwrap_or(50),
            error_handling_mode: claw_config
                .error_handling_mode
                .clone()
                .unwrap_or(config::ErrorHandlingMode::Flexible),
            excluded_directories: claw_config
                .excluded_directories
                .clone()
                .unwrap_or_else(|| vec![".git".to_string(), "node_modules".to_string(), "target".to_string()]),
            excluded_extensions: claw_config
                .excluded_extensions
                .clone()
                .unwrap_or_else(|| vec!["exe".to_string(), "bin".to_string(), "so".to_string()]),
        };

        let files = context::discover_files(&context_config)?;
        let result = context::validate_and_read_files(files, &context_config);

        // Handle errors based on mode
        context::handle_errors(&result, &context_config.error_handling_mode)?;

        // Format and append to prompt
        let context_section = context::format_context(&result, &context_config);
        rendered_prompt.push_str("\n\n");
        rendered_prompt.push_str(&context_section);
    }

    runner::run_llm(claw_config, &rendered_prompt)?;

    Ok(())
}
