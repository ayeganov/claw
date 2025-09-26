use crate::config::{self, ClawConfig};
use crate::runner;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tera::Context as TeraContext;
use tera::Tera;

pub fn handle_add_command(
    name: &str,
    local: bool,
    global: bool,
    claw_config: &ClawConfig,
) -> Result<()> {
    // 1. Determine the final, unambiguous save path based on flags.
    let paths = config::ConfigPaths::new()?;
    let save_dir_base = match (local, global) {
        (true, false) => {
            let local_path = paths.local.unwrap_or_else(|| PathBuf::from(".claw"));
            fs::create_dir_all(&local_path).with_context(|| {
                format!(
                    "Failed to create local directory at {}",
                    local_path.display()
                )
            })?;
            println!(
                "--local flag used. Goal will be saved in: {}",
                local_path.display()
            );
            local_path
        }
        (false, true) => {
            let global_path = paths.global.unwrap();
            println!(
                "--global flag used. Goal will be saved in: {}",
                global_path.display()
            );
            global_path
        }
        (false, false) => paths.local.unwrap_or_else(|| paths.global.unwrap()),
        (true, true) => unreachable!(),
    };

    let save_path = save_dir_base.join("goals").join(name);

    // 2. Prepare and render the meta-prompt.
    let mut context = TeraContext::new();
    context.insert("save_path", &save_path.display().to_string());
    context.insert("goal_name", &name);

    const META_PROMPT_TEMPLATE: &str = include_str!("../../prompts/add_meta_prompt.txt");

    // The entire file is the template. Render it directly.
    let rendered_meta_prompt = Tera::one_off(META_PROMPT_TEMPLATE, &context, false)
        .context("Failed to render the 'add' command meta-prompt.")?;

    // 3. Handoff to the LLM agent.
    println!("\nStarting agent session to create goal '{}'...", name);
    println!("The agent will create files in: {}", save_path.display());
    println!("Please follow the instructions from the assistant.");

    runner::run_llm(claw_config, &rendered_meta_prompt)?;

    println!("\nAgent session finished. Verify that the goal was created successfully.");
    Ok(())
}
