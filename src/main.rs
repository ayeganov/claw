mod cli;
mod commands;
mod config;
mod runner;

use anyhow::{Context as AnyhowContext, Result};
use clap::Parser;
use cli::{Cli, Subcommands};
use dialoguer::{Select, theme::ColorfulTheme};
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
                run_goal(&goal_name, &claw_config, &cli.run_args.template_args)?;
            } else {
                // No goal was provided, so enter interactive mode.
                let goals = config::find_all_goals()?;
                if goals.is_empty() {
                    anyhow::bail!("No goals found. Add a goal using `claw add <goal_name>`.");
                }

                // Create a formatted list for the selection menu
                let items: Vec<String> = goals
                    .iter()
                    .map(|g| {
                        let description = g.config.description.as_deref().unwrap_or("");
                        format!(
                            "{:<20} -- {:<40} ({})", // Format for alignment
                            g.config.name, description, g.source
                        )
                    })
                    .collect();

                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Choose a goal to run")
                    .items(&items)
                    .default(0)
                    .interact()?;

                let selected_goal_name = &goals[selection].name;

                run_goal(selected_goal_name, &claw_config, &Vec::new())?;
            }
        }
    }

    Ok(())
}

fn run_goal(
    goal_name: &str,
    claw_config: &config::ClawConfig,
    template_args: &[String],
) -> Result<()> {
    let goal = config::find_and_load_goal(goal_name)?;
    let script_outputs = runner::execute_context_scripts(&goal.config.context_scripts)?;
    let template_args = cli::parse_template_args(template_args)?;

    let mut context = Context::new();
    context.insert("Args", &template_args);
    context.insert("Context", &script_outputs);

    let mut tera = Tera::new(&format!("{}/**/*", goal.directory.display()))
        .context("Failed to create Tera instance")?;
    tera.add_raw_template("prompt", &goal.config.prompt)
        .context("Failed to add raw template")?;
    let rendered_prompt = tera
        .render("prompt", &context)
        .map_err(|e| anyhow::anyhow!("Failed to render prompt for goal '{}': {}", goal_name, e))?;

    runner::run_llm(claw_config, &rendered_prompt)?;

    Ok(())
}
