use crate::config::{GoalParameter, LoadedGoal, ParameterType};

/// Formats help text for a goal with parameters.
pub fn format_goal_help(goal: &LoadedGoal, goal_name: &str) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("Goal: {} ({})\n", goal.config.name, goal_name));
    if let Some(desc) = &goal.config.description {
        output.push_str(&format!("Description: {}\n", desc));
    }
    output.push('\n');

    // If there are no parameters, just show basic usage
    if goal.config.parameters.is_empty() {
        output.push_str("Usage:\n");
        output.push_str(&format!(
            "  claw {} [--context <path>] [-- <options>]\n\n",
            goal_name
        ));
        output.push_str("This goal didn't define any parameters.\n");
        output.push_str("Parameters are passed after '--' as --key value or --key=value.\n");
        output.push_str("\nTo see this help again, run:\n");
        output.push_str(&format!("  claw {} --explain\n", goal_name));
        return output;
    }

    // Separate required and optional parameters
    let required: Vec<&GoalParameter> = goal
        .config
        .parameters
        .iter()
        .filter(|p| p.required)
        .collect();

    let optional: Vec<&GoalParameter> = goal
        .config
        .parameters
        .iter()
        .filter(|p| !p.required)
        .collect();

    // Show required parameters
    if !required.is_empty() {
        output.push_str("Required Parameters:\n");
        for param in &required {
            output.push_str(&format_parameter(param));
            output.push('\n');
        }
    }

    // Show optional parameters
    if !optional.is_empty() {
        output.push_str("Optional Parameters:\n");
        for param in &optional {
            output.push_str(&format_parameter(param));
            output.push('\n');
        }
    }

    // Show built-in claw flags
    output.push_str("Built-in Claw Flags:\n");
    output.push_str("  -c, --context <path>       Files or directories to include as context\n");
    output.push_str(
        "  -d, --recurse_depth <num>  Maximum recursion depth when scanning directories\n",
    );
    output.push_str("  -e, --explain              Show this help information\n");
    output.push('\n');

    // Show usage examples
    output.push_str("Usage Examples:\n");
    output.push_str(&format!("  claw {} --", goal_name));
    for param in &required {
        output.push_str(&format!(" --{} <value>", param.name));
    }
    output.push('\n');

    if !optional.is_empty() || !required.is_empty() {
        output.push_str(&format!("  claw {} --context ./src --", goal_name));
        for param in required.iter().take(1) {
            output.push_str(&format!(" --{} <value>", param.name));
        }
        if let Some(first_optional) = optional.first() {
            output.push_str(&format!(" --{} <value>", first_optional.name));
        }
        output.push('\n');
    }

    output
}

/// Formats a single parameter for display.
fn format_parameter(param: &GoalParameter) -> String {
    let mut output = String::new();

    // Parameter name and type
    output.push_str("  --");
    output.push_str(&param.name);
    if let Some(param_type) = &param.param_type {
        output.push_str(&format!(" <{}>", format_type(param_type)));
    }

    // Show default value if present
    if let Some(default) = &param.default {
        output.push_str(&format!("  (default: \"{}\")", default));
    }
    output.push('\n');

    // Description with proper indentation
    let description_lines = wrap_text(&param.description, 70);
    for line in description_lines {
        output.push_str("      ");
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// Formats a parameter type for display.
fn format_type(param_type: &ParameterType) -> String {
    match param_type {
        ParameterType::String => "string".to_string(),
        ParameterType::Number => "number".to_string(),
        ParameterType::Boolean => "boolean".to_string(),
    }
}

/// Wraps text to a maximum width, breaking on word boundaries.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + word.len() + 1 <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PromptConfig;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_param(
        name: &str,
        description: &str,
        required: bool,
        param_type: Option<ParameterType>,
        default: Option<&str>,
    ) -> GoalParameter {
        GoalParameter {
            name: name.to_string(),
            description: description.to_string(),
            required,
            param_type,
            default: default.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_goal_without_parameters() {
        let goal = LoadedGoal {
            config: PromptConfig {
                name: "Test Goal".to_string(),
                description: Some("A test goal".to_string()),
                parameters: Vec::new(),
                context_scripts: HashMap::new(),
                prompt: "test".to_string(),
            },
            directory: PathBuf::from("/test"),
        };

        let help = format_goal_help(&goal, "test-goal");
        assert!(help.contains("Test Goal"));
        assert!(help.contains("A test goal"));
        assert!(help.contains("accepts arbitrary parameters"));
    }

    #[test]
    fn test_goal_with_required_parameters() {
        let goal = LoadedGoal {
            config: PromptConfig {
                name: "Test Goal".to_string(),
                description: Some("A test goal".to_string()),
                parameters: vec![create_test_param(
                    "scope",
                    "The scope of the review",
                    true,
                    Some(ParameterType::String),
                    None,
                )],
                context_scripts: HashMap::new(),
                prompt: "test".to_string(),
            },
            directory: PathBuf::from("/test"),
        };

        let help = format_goal_help(&goal, "test-goal");
        assert!(help.contains("Required Parameters"));
        assert!(help.contains("--scope"));
        assert!(help.contains("The scope of the review"));
    }

    #[test]
    fn test_goal_with_optional_parameters() {
        let goal = LoadedGoal {
            config: PromptConfig {
                name: "Test Goal".to_string(),
                description: None,
                parameters: vec![create_test_param(
                    "format",
                    "Output format",
                    false,
                    Some(ParameterType::String),
                    Some("markdown"),
                )],
                context_scripts: HashMap::new(),
                prompt: "test".to_string(),
            },
            directory: PathBuf::from("/test"),
        };

        let help = format_goal_help(&goal, "test-goal");
        assert!(help.contains("Optional Parameters"));
        assert!(help.contains("--format"));
        assert!(help.contains("default: \"markdown\""));
    }

    #[test]
    fn test_wrap_text() {
        let text = "This is a long description that should be wrapped at the specified width";
        let wrapped = wrap_text(text, 30);
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(line.len() <= 30);
        }
    }

    #[test]
    fn test_format_type() {
        assert_eq!(format_type(&ParameterType::String), "string");
        assert_eq!(format_type(&ParameterType::Number), "number");
        assert_eq!(format_type(&ParameterType::Boolean), "boolean");
    }
}
