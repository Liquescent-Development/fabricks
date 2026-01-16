//! Variable resolution for mortar files.
//!
//! Resolves `${variable.name}` and `${variable.name:-default}` references
//! in mortar file strings, particularly environment variables.

use std::collections::HashMap;

use regex::Regex;

use crate::error::{CommonError, Result};
use crate::models::mortar::{MortarFile, Variable, VariableType};

/// A resolved variable value.
#[derive(Debug, Clone, PartialEq)]
pub enum VariableValue {
    /// String value.
    String(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Boolean(bool),
}

impl VariableValue {
    /// Converts the value to a string representation.
    #[must_use]
    pub fn as_string(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::Number(n) => n.to_string(),
            Self::Boolean(b) => b.to_string(),
        }
    }
}

impl From<&Variable> for Option<VariableValue> {
    fn from(var: &Variable) -> Self {
        var.default.as_ref().map(|default| match var.var_type {
            VariableType::String => {
                VariableValue::String(default.as_str().unwrap_or_default().to_string())
            }
            VariableType::Number => VariableValue::Number(default.as_float().unwrap_or(0.0)),
            VariableType::Boolean => VariableValue::Boolean(default.as_bool().unwrap_or(false)),
        })
    }
}

/// Resolves variable references in strings.
///
/// Supports two syntaxes:
/// - `${variable.name}` - Substitutes with variable value, errors if undefined
/// - `${variable.name:-default}` - Uses default value if variable is undefined
#[derive(Debug)]
pub struct VariableResolver {
    variables: HashMap<String, VariableValue>,
}

impl VariableResolver {
    /// Creates a new resolver with the given variables.
    #[must_use]
    pub fn new(variables: HashMap<String, VariableValue>) -> Self {
        Self { variables }
    }

    /// Creates a resolver from a mortar file's variable definitions.
    ///
    /// Extracts default values from the variable definitions.
    #[must_use]
    pub fn from_mortar(mortar: &MortarFile) -> Self {
        let variables = mortar
            .variable
            .as_ref()
            .map(|vars| {
                vars.iter()
                    .filter_map(|(name, var)| {
                        let value: Option<VariableValue> = var.into();
                        value.map(|v| (name.clone(), v))
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self { variables }
    }

    /// Creates a resolver with explicit variable overrides.
    ///
    /// Overrides take precedence over defaults from the mortar file.
    #[must_use]
    pub fn from_mortar_with_overrides(
        mortar: &MortarFile,
        overrides: HashMap<String, String>,
    ) -> Self {
        let mut resolver = Self::from_mortar(mortar);

        // Apply overrides
        for (name, value) in overrides {
            resolver
                .variables
                .insert(name, VariableValue::String(value));
        }

        resolver
    }

    /// Resolves all variable references in a string.
    ///
    /// # Syntax
    ///
    /// - `${variable.name}` - Replaced with variable value
    /// - `${variable.name:-default}` - Uses default if variable undefined
    ///
    /// # Errors
    ///
    /// Returns an error if a variable reference cannot be resolved and has
    /// no default value.
    pub fn resolve(&self, input: &str) -> Result<String> {
        // Pattern matches ${variable.name} or ${variable.name:-default}
        // This pattern is a compile-time constant and will always succeed
        let pattern = Regex::new(r"\$\{variable\.([a-zA-Z_][a-zA-Z0-9_]*)(?::-([^}]*))?\}")
            .map_err(|e| CommonError::VariableResolution(format!("regex error: {e}")))?;

        let mut result = input.to_string();
        let mut errors: Vec<String> = Vec::new();

        // Find all matches and collect them (to avoid borrowing issues)
        let matches: Vec<_> = pattern
            .captures_iter(input)
            .filter_map(|cap| {
                // Group 0 (full match) and group 1 (variable name) are always present
                // when captures_iter returns a match
                let full_match = cap.get(0)?.as_str().to_string();
                let var_name = cap.get(1)?.as_str().to_string();
                let default_value = cap.get(2).map(|m| m.as_str().to_string());
                Some((full_match, var_name, default_value))
            })
            .collect();

        for (full_match, var_name, default_value) in matches {
            let replacement = if let Some(value) = self.variables.get(&var_name) {
                value.as_string()
            } else if let Some(default) = default_value {
                default
            } else {
                errors.push(format!("undefined variable: {var_name}"));
                continue;
            };

            result = result.replace(&full_match, &replacement);
        }

        if !errors.is_empty() {
            return Err(CommonError::VariableResolution(errors.join(", ")));
        }

        Ok(result)
    }

    /// Resolves variables in an environment map.
    ///
    /// # Errors
    ///
    /// Returns an error if any variable reference cannot be resolved.
    pub fn resolve_environment(
        &self,
        env: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        env.iter()
            .map(|(k, v)| Ok((k.clone(), self.resolve(v)?)))
            .collect()
    }

    /// Gets a variable value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&VariableValue> {
        self.variables.get(name)
    }

    /// Checks if a variable is defined.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_resolver() -> VariableResolver {
        let mut vars = HashMap::new();
        vars.insert(
            "log_level".to_string(),
            VariableValue::String("info".to_string()),
        );
        vars.insert("db_pool_size".to_string(), VariableValue::Number(10.0));
        vars.insert("debug".to_string(), VariableValue::Boolean(false));
        VariableResolver::new(vars)
    }

    #[test]
    fn test_resolve_string_variable() {
        let resolver = make_resolver();
        let result = resolver.resolve("LOG_LEVEL=${variable.log_level}").unwrap();
        assert_eq!(result, "LOG_LEVEL=info");
    }

    #[test]
    fn test_resolve_number_variable() {
        let resolver = make_resolver();
        let result = resolver
            .resolve("pool_size=${variable.db_pool_size}")
            .unwrap();
        assert_eq!(result, "pool_size=10");
    }

    #[test]
    fn test_resolve_boolean_variable() {
        let resolver = make_resolver();
        let result = resolver.resolve("DEBUG=${variable.debug}").unwrap();
        assert_eq!(result, "DEBUG=false");
    }

    #[test]
    fn test_resolve_with_default() {
        let resolver = make_resolver();
        let result = resolver.resolve("TIMEOUT=${variable.timeout:-30}").unwrap();
        assert_eq!(result, "TIMEOUT=30");
    }

    #[test]
    fn test_resolve_defined_ignores_default() {
        let resolver = make_resolver();
        let result = resolver
            .resolve("LOG=${variable.log_level:-debug}")
            .unwrap();
        assert_eq!(result, "LOG=info");
    }

    #[test]
    fn test_resolve_undefined_no_default_errors() {
        let resolver = make_resolver();
        let result = resolver.resolve("VALUE=${variable.undefined}");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_multiple_variables() {
        let resolver = make_resolver();
        let result = resolver
            .resolve("LOG=${variable.log_level}, POOL=${variable.db_pool_size}")
            .unwrap();
        assert_eq!(result, "LOG=info, POOL=10");
    }

    #[test]
    fn test_resolve_no_variables() {
        let resolver = make_resolver();
        let result = resolver.resolve("plain string").unwrap();
        assert_eq!(result, "plain string");
    }

    #[test]
    fn test_resolve_environment() {
        let resolver = make_resolver();
        let mut env = HashMap::new();
        env.insert("LOG_LEVEL".to_string(), "${variable.log_level}".to_string());
        env.insert(
            "POOL_SIZE".to_string(),
            "${variable.db_pool_size}".to_string(),
        );
        env.insert("STATIC".to_string(), "unchanged".to_string());

        let resolved = resolver.resolve_environment(&env).unwrap();

        assert_eq!(resolved.get("LOG_LEVEL"), Some(&"info".to_string()));
        assert_eq!(resolved.get("POOL_SIZE"), Some(&"10".to_string()));
        assert_eq!(resolved.get("STATIC"), Some(&"unchanged".to_string()));
    }

    #[test]
    fn test_variable_value_as_string() {
        assert_eq!(
            VariableValue::String("hello".to_string()).as_string(),
            "hello"
        );
        assert_eq!(VariableValue::Number(42.5).as_string(), "42.5");
        assert_eq!(VariableValue::Boolean(true).as_string(), "true");
    }

    #[test]
    fn test_default_with_colon() {
        let resolver = make_resolver();
        let result = resolver
            .resolve("URL=${variable.url:-http://localhost:8080}")
            .unwrap();
        assert_eq!(result, "URL=http://localhost:8080");
    }

    #[test]
    fn test_empty_default() {
        let resolver = make_resolver();
        let result = resolver.resolve("VALUE=${variable.empty:-}").unwrap();
        assert_eq!(result, "VALUE=");
    }
}
