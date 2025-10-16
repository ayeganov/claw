mod cli;
mod commands;
mod config;
mod context;
mod goal_browser;
mod help;
mod runner;
mod validation;

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
        Some(Subcommands::List { local, global }) => {
            commands::list::handle_list_command(local, global)?;
        }
        Some(Subcommands::Pass) => {
            runner::run_pass_through(&claw_config)?;
        }
        Some(Subcommands::DryRun {
            goal_name,
            output,
            common,
        }) => {
            let rendered_prompt = render_goal_prompt(
                &goal_name,
                &claw_config,
                &common.template_args,
                &common.context,
                common.recurse_depth,
            )?;

            commands::dry_run::handle_dry_run_command(output.as_ref(), &rendered_prompt)?;
        }
        None => {
            if let Some(goal_name) = cli.run_args.goal_name {
                // Check for --explain flag
                if cli.run_args.explain {
                    // Show goal-specific help
                    let goal = config::find_and_load_goal(&goal_name)?;
                    let help_text = help::format_goal_help(&goal, &goal_name);
                    println!("{}", help_text);
                    return Ok(());
                }

                run_goal(
                    &goal_name,
                    &claw_config,
                    &cli.run_args.common.template_args,
                    &cli.run_args.common.context,
                    cli.run_args.common.recurse_depth,
                )?;
            } else {
                println!("No goal given");
                commands::list::handle_list_command(false, false)?;
                // No goal was provided, so enter interactive mode.
                //                let goals = config::find_all_goals()?;
                //                if goals.is_empty() {
                //                    anyhow::bail!("No goals found. Add a goal using `claw add <goal_name>`.");
                //                }
                //
                //                // Use the new goal browser TUI
                //                let selected_goal_name = goal_browser::run_goal_browser(goals)?;
                //
                //                run_goal(&selected_goal_name, &claw_config, &Vec::new(), &Vec::new(), None)?;
            }
        }
    }

    Ok(())
}

/// Parses goal arguments into a HashMap.
/// Supports formats: `--key=value`, `--key value`, and `--flag` (boolean).
fn parse_goal_args(args: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--") {
            anyhow::bail!(
                "Invalid goal argument: '{}'. All goal arguments must be flags starting with '--'.",
                arg
            );
        }

        let key_part = &arg[2..]; // Remove the "--"
        if let Some((key, value)) = key_part.split_once('=') {
            // Handles --key=value
            map.insert(key.to_string(), value.to_string());
            i += 1;
        } else {
            // Handles --key value or --flag (boolean)
            i += 1;
            if i >= args.len() || args[i].starts_with("--") {
                // This is a boolean flag (no value provided)
                map.insert(key_part.to_string(), "true".to_string());
            } else {
                // This has a value
                let value = &args[i];
                map.insert(key_part.to_string(), value.to_string());
                i += 1;
            }
        }
    }
    Ok(map)
}

/// Renders a goal's prompt with all context, scripts, and file context applied.
///
/// This function performs all the steps needed to generate the final prompt that
/// would be sent to the LLM, including:
/// - Loading and validating the goal
/// - Parsing and validating template arguments
/// - Executing context scripts
/// - Rendering the prompt template with Tera
/// - Adding file context if specified
///
/// # Arguments
/// * `goal_name` - Name of the goal to render
/// * `claw_config` - Configuration for context settings
/// * `template_args` - Template arguments from command line
/// * `context_paths` - File paths to include as context
/// * `recurse_depth` - Directory recursion depth
///
/// # Returns
/// * `Ok(String)` - The fully rendered prompt
/// * `Err` - If any step fails (goal not found, validation errors, script failures, etc.)
fn render_goal_prompt(
    goal_name: &str,
    claw_config: &config::ClawConfig,
    template_args: &[String],
    context_paths: &[std::path::PathBuf],
    recurse_depth: Option<usize>,
) -> Result<String> {
    let goal = config::find_and_load_goal(goal_name)?;

    // Parse template args into HashMap
    let parsed_args = parse_goal_args(template_args)?;

    // Validate parameters against the goal's parameter definitions
    let validator =
        validation::ParameterValidator::new(&goal.config.parameters, goal_name.to_string());
    let template_args = validator.validate(&parsed_args)?;

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
            excluded_directories: claw_config.excluded_directories.clone().unwrap_or_else(|| {
                vec![
                    ".git".to_string(),
                    "node_modules".to_string(),
                    "target".to_string(),
                ]
            }),
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

    Ok(rendered_prompt)
}

fn run_goal(
    goal_name: &str,
    claw_config: &config::ClawConfig,
    template_args: &[String],
    context_paths: &[std::path::PathBuf],
    recurse_depth: Option<usize>,
) -> Result<()> {
    let rendered_prompt = render_goal_prompt(
        goal_name,
        claw_config,
        template_args,
        context_paths,
        recurse_depth,
    )?;

    // Check for large prompt warning
    runner::check_prompt_size_warning(&rendered_prompt, &claw_config.prompt_arg_template);

    // Create receiver and send prompt
    let receiver = runner::create_receiver(claw_config);
    receiver.send_prompt(&rendered_prompt)?;

    Ok(())
}
