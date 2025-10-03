use crate::config::GoalParameter;
use anyhow::Result;
use std::collections::HashMap;

/// Validates parameters against a goal's parameter definitions.
pub struct ParameterValidator<'a> {
    parameters: &'a [GoalParameter],
    goal_name: String,
}

/// Represents errors that occur during parameter validation.
#[derive(Debug)]
pub struct ValidationError {
    pub missing_params: Vec<GoalParameter>,
    pub goal_name: String,
}

impl<'a> ParameterValidator<'a> {
    /// Creates a new parameter validator for the given goal.
    pub fn new(parameters: &'a [GoalParameter], goal_name: String) -> Self {
        Self {
            parameters,
            goal_name,
        }
    }

    /// Validates the provided arguments against the goal's parameter definitions.
    /// Returns a HashMap with all parameters (including defaults) if validation succeeds.
    pub fn validate(&self, args: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // If there are no parameter definitions, accept all arguments as-is
        if self.parameters.is_empty() {
            return Ok(args.clone());
        }

        let missing = self.get_missing_required(args);
        if !missing.is_empty() {
            anyhow::bail!(ValidationError {
                missing_params: missing,
                goal_name: self.goal_name.clone(),
            });
        }

        // Build the final parameter map with defaults applied
        let mut result = args.clone();
        for param in self.parameters {
            if !result.contains_key(&param.name) {
                if let Some(default) = &param.default {
                    result.insert(param.name.clone(), default.clone());
                }
            }
        }

        Ok(result)
    }

    /// Returns a list of required parameters that are missing from the provided arguments.
    pub fn get_missing_required(&self, args: &HashMap<String, String>) -> Vec<GoalParameter> {
        self.parameters
            .iter()
            .filter(|p| p.required && !args.contains_key(&p.name))
            .cloned()
            .collect()
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Goal '{}' is missing required parameters:",
            self.goal_name
        )?;
        writeln!(f)?;
        for param in &self.missing_params {
            write!(f, "  --{}", param.name)?;
            if let Some(param_type) = &param.param_type {
                write!(f, " <{:?}>", param_type)?;
            }
            writeln!(f)?;
            writeln!(f, "      {}", param.description)?;
        }
        writeln!(f)?;
        writeln!(f, "Run 'claw {} --explain' for more information.", self.goal_name)?;
        Ok(())
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ParameterType;

    fn create_test_param(name: &str, required: bool, default: Option<&str>) -> GoalParameter {
        GoalParameter {
            name: name.to_string(),
            description: format!("Description for {}", name),
            required,
            param_type: Some(ParameterType::String),
            default: default.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_no_parameters_accepts_all() {
        let validator = ParameterValidator::new(&[], "test-goal".to_string());
        let mut args = HashMap::new();
        args.insert("anything".to_string(), "value".to_string());

        let result = validator.validate(&args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().get("anything"), Some(&"value".to_string()));
    }

    #[test]
    fn test_missing_required_parameter() {
        let params = vec![create_test_param("scope", true, None)];
        let validator = ParameterValidator::new(&params, "test-goal".to_string());
        let args = HashMap::new();

        let result = validator.validate(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_required_parameters_provided() {
        let params = vec![create_test_param("scope", true, None)];
        let validator = ParameterValidator::new(&params, "test-goal".to_string());
        let mut args = HashMap::new();
        args.insert("scope".to_string(), "auth".to_string());

        let result = validator.validate(&args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().get("scope"), Some(&"auth".to_string()));
    }

    #[test]
    fn test_default_value_applied() {
        let params = vec![create_test_param("format", false, Some("markdown"))];
        let validator = ParameterValidator::new(&params, "test-goal".to_string());
        let args = HashMap::new();

        let result = validator.validate(&args);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.get("format"), Some(&"markdown".to_string()));
    }

    #[test]
    fn test_provided_value_overrides_default() {
        let params = vec![create_test_param("format", false, Some("markdown"))];
        let validator = ParameterValidator::new(&params, "test-goal".to_string());
        let mut args = HashMap::new();
        args.insert("format".to_string(), "json".to_string());

        let result = validator.validate(&args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().get("format"), Some(&"json".to_string()));
    }
}
