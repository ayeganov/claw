use crate::config::{find_all_goals, ConfigPaths, DiscoveredGoal, GoalSource};
use anyhow::Result;

/// Handles the `claw list` command.
pub fn handle_list_command(show_local_only: bool, show_global_only: bool) -> Result<()> {
    let paths = ConfigPaths::new()?;
    let goals = find_all_goals()?;

    if goals.is_empty() {
        println!("No goals found.");
        println!("Add a goal using: claw add <goal_name>");
        return Ok(());
    }

    // Filter goals based on flags
    let local_goals: Vec<&DiscoveredGoal> = goals
        .iter()
        .filter(|g| g.source == GoalSource::Local)
        .collect();

    let global_goals: Vec<&DiscoveredGoal> = goals
        .iter()
        .filter(|g| g.source == GoalSource::Global)
        .collect();

    // Display local goals
    if !show_global_only && !local_goals.is_empty() {
        let local_path = paths
            .local
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "./.claw/".to_string());
        println!("Local Goals ({}):", local_path);
        println!();
        for goal in &local_goals {
            print_goal_info(goal);
        }
    }

    // Display global goals
    if !show_local_only && !global_goals.is_empty() {
        if !show_global_only && !local_goals.is_empty() {
            println!(); // Separator between sections
        }
        let global_path = paths
            .global
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.config/claw/".to_string());
        println!("Global Goals ({}):", global_path);
        println!();
        for goal in &global_goals {
            print_goal_info(goal);
        }
    }

    Ok(())
}

/// Prints information about a single goal.
fn print_goal_info(goal: &DiscoveredGoal) {
    // CLI name - human name
    println!("  {} - {}", goal.name, goal.config.name);

    // Description (indented)
    if let Some(desc) = &goal.config.description {
        println!("    {}", desc);
    }

    // Parameter count
    let required_count = goal.config.parameters.iter().filter(|p| p.required).count();
    let optional_count = goal.config.parameters.iter().filter(|p| !p.required).count();

    if goal.config.parameters.is_empty() {
        println!("    Parameters: accepts arbitrary parameters");
    } else if required_count > 0 && optional_count > 0 {
        println!(
            "    Parameters: {} required, {} optional",
            required_count, optional_count
        );
    } else if required_count > 0 {
        println!("    Parameters: {} required", required_count);
    } else {
        println!("    Parameters: {} optional", optional_count);
    }

    println!(); // Blank line between goals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GoalParameter, ParameterType, PromptConfig};
    use std::collections::HashMap;

    fn create_test_goal_with_params(
        name: &str,
        source: GoalSource,
        required: usize,
        optional: usize,
    ) -> DiscoveredGoal {
        let mut parameters = Vec::new();

        for i in 0..required {
            parameters.push(GoalParameter {
                name: format!("req{}", i),
                description: "Required param".to_string(),
                required: true,
                param_type: Some(ParameterType::String),
                default: None,
            });
        }

        for i in 0..optional {
            parameters.push(GoalParameter {
                name: format!("opt{}", i),
                description: "Optional param".to_string(),
                required: false,
                param_type: Some(ParameterType::String),
                default: Some("default".to_string()),
            });
        }

        DiscoveredGoal {
            name: name.to_string(),
            source,
            config: PromptConfig {
                name: format!("{} Display Name", name),
                description: Some(format!("{} description", name)),
                parameters,
                context_scripts: HashMap::new(),
                prompt: "test".to_string(),
            },
        }
    }

    #[test]
    fn test_print_goal_info_no_params() {
        let goal = create_test_goal_with_params("test", GoalSource::Local, 0, 0);
        // Just ensure it doesn't panic
        print_goal_info(&goal);
    }

    #[test]
    fn test_print_goal_info_with_params() {
        let goal = create_test_goal_with_params("test", GoalSource::Local, 2, 1);
        // Just ensure it doesn't panic
        print_goal_info(&goal);
    }
}
