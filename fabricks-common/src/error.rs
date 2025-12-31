//! Error types for the Fabricks common library.

use thiserror::Error;

/// Errors that can occur during validation.
#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    /// The file format version is not supported.
    #[error("unsupported version: {version}, expected: {expected}")]
    UnsupportedVersion {
        /// The version found in the file.
        version: String,
        /// The expected version.
        expected: String,
    },

    /// The name format is invalid.
    #[error(
        "invalid name '{name}': must match pattern [a-z0-9-]+ (lowercase letters, numbers, and hyphens only)"
    )]
    InvalidName {
        /// The invalid name.
        name: String,
    },

    /// The version format is invalid (not semver).
    #[error("invalid version '{version}': must be valid semantic version (MAJOR.MINOR.PATCH)")]
    InvalidVersion {
        /// The invalid version string.
        version: String,
    },

    /// A port number is out of valid range.
    #[error("invalid port {port}: must be between 1 and 65535")]
    InvalidPort {
        /// The invalid port number.
        port: u32,
    },

    /// A path is invalid.
    #[error("invalid path '{path}': {reason}")]
    InvalidPath {
        /// The invalid path.
        path: String,
        /// Why the path is invalid.
        reason: String,
    },

    /// A URL is invalid.
    #[error("invalid URL '{url}': {reason}")]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// Why the URL is invalid.
        reason: String,
    },

    /// A required field is missing.
    #[error("missing required field: {field}")]
    MissingField {
        /// The name of the missing field.
        field: String,
    },

    /// Mutually exclusive options were both specified.
    #[error("mutually exclusive options: '{option1}' and '{option2}' cannot both be specified")]
    MutuallyExclusive {
        /// The first conflicting option.
        option1: String,
        /// The second conflicting option.
        option2: String,
    },

    /// A circular dependency was detected.
    #[error("circular dependency detected: {cycle}")]
    CircularDependency {
        /// Description of the cycle.
        cycle: String,
    },

    /// A reference to a non-existent entity.
    #[error("{entity_type} '{name}' not found")]
    NotFound {
        /// Type of entity (e.g., "network", "service", "volume").
        entity_type: String,
        /// Name of the entity.
        name: String,
    },

    /// A duplicate definition was found.
    #[error("duplicate {entity_type}: '{name}'")]
    Duplicate {
        /// Type of entity.
        entity_type: String,
        /// Name of the duplicate.
        name: String,
    },

    /// A duration string could not be parsed.
    #[error("invalid duration '{value}': expected format like '30s', '5m', '1h'")]
    InvalidDuration {
        /// The invalid duration string.
        value: String,
    },

    /// A byte size string could not be parsed.
    #[error("invalid byte size '{value}': expected format like '256Mi', '1Gi', '500Ki'")]
    InvalidByteSize {
        /// The invalid byte size string.
        value: String,
    },

    /// An image reference is invalid.
    #[error("invalid image reference '{image}': {reason}")]
    InvalidImageReference {
        /// The invalid image reference.
        image: String,
        /// Why it's invalid.
        reason: String,
    },

    /// A host:port specification is invalid.
    #[error("invalid host:port '{value}': {reason}")]
    InvalidHostPort {
        /// The invalid value.
        value: String,
        /// Why it's invalid.
        reason: String,
    },

    /// A cron expression is invalid.
    #[error("invalid cron expression '{value}': {reason}")]
    InvalidCronExpression {
        /// The invalid cron expression.
        value: String,
        /// Why it's invalid.
        reason: String,
    },

    /// Multiple validation errors occurred.
    #[error("multiple validation errors:\n{}", format_errors(.0))]
    Multiple(Vec<ValidationError>),
}

/// Format multiple errors for display.
fn format_errors(errors: &[ValidationError]) -> String {
    errors
        .iter()
        .enumerate()
        .map(|(i, e)| format!("  {}. {e}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Errors that can occur during parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Failed to read the file.
    #[error("failed to read file '{path}': {source}")]
    IoError {
        /// The file path.
        path: String,
        /// The underlying IO error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse TOML.
    #[error("failed to parse TOML: {source}")]
    TomlError {
        /// The underlying TOML error.
        #[source]
        source: toml::de::Error,
    },

    /// Validation failed after parsing.
    #[error("validation failed: {source}")]
    ValidationError {
        /// The validation error.
        #[source]
        source: ValidationError,
    },
}

impl From<toml::de::Error> for ParseError {
    fn from(source: toml::de::Error) -> Self {
        Self::TomlError { source }
    }
}

impl From<ValidationError> for ParseError {
    fn from(source: ValidationError) -> Self {
        Self::ValidationError { source }
    }
}
