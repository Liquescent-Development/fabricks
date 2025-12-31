//! Common types used across Fabrickfile and `MortarFile`.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ValidationError;

/// A duration value parsed from strings like "30s", "5m", "1h".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    /// Duration in seconds.
    seconds: u64,
}

impl Duration {
    /// Creates a new duration from seconds.
    #[must_use]
    pub const fn from_secs(seconds: u64) -> Self {
        Self { seconds }
    }

    /// Returns the duration as seconds.
    #[must_use]
    pub const fn as_secs(&self) -> u64 {
        self.seconds
    }

    /// Returns the duration as a `std::time::Duration`.
    #[must_use]
    pub const fn as_std(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.seconds)
    }
}

impl FromStr for Duration {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ValidationError::InvalidDuration {
                value: s.to_string(),
            });
        }

        // Find where the number ends and the unit begins
        let (num_str, unit) = s
            .find(|c: char| !c.is_ascii_digit())
            .map_or((s, ""), |i| s.split_at(i));

        let num: u64 = num_str
            .parse()
            .map_err(|_| ValidationError::InvalidDuration {
                value: s.to_string(),
            })?;

        let multiplier = match unit {
            "s" | "sec" | "second" | "seconds" | "" => 1,
            "m" | "min" | "minute" | "minutes" => 60,
            "h" | "hr" | "hour" | "hours" => 3_600,
            "d" | "day" | "days" => 86_400,
            _ => {
                return Err(ValidationError::InvalidDuration {
                    value: s.to_string(),
                });
            }
        };

        Ok(Self {
            seconds: num * multiplier,
        })
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const DAY: u64 = 86_400;
        const HOUR: u64 = 3_600;
        const MINUTE: u64 = 60;

        if self.seconds.is_multiple_of(DAY) && self.seconds >= DAY {
            write!(f, "{}d", self.seconds / DAY)
        } else if self.seconds.is_multiple_of(HOUR) && self.seconds >= HOUR {
            write!(f, "{}h", self.seconds / HOUR)
        } else if self.seconds.is_multiple_of(MINUTE) && self.seconds >= MINUTE {
            write!(f, "{}m", self.seconds / MINUTE)
        } else {
            write!(f, "{}s", self.seconds)
        }
    }
}

impl Serialize for Duration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// A byte size value parsed from strings like "256Mi", "1Gi", "500Ki".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteSize {
    /// Size in bytes.
    bytes: u64,
}

impl ByteSize {
    /// Creates a new byte size from bytes.
    #[must_use]
    pub const fn from_bytes(bytes: u64) -> Self {
        Self { bytes }
    }

    /// Returns the size in bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> u64 {
        self.bytes
    }

    /// Returns the size in kibibytes (1024 bytes).
    #[must_use]
    pub const fn as_kibibytes(&self) -> u64 {
        self.bytes / 1024
    }

    /// Returns the size in mebibytes (1024^2 bytes).
    #[must_use]
    pub const fn as_mebibytes(&self) -> u64 {
        self.bytes / (1024 * 1024)
    }

    /// Returns the size in gibibytes (1024^3 bytes).
    #[must_use]
    pub const fn as_gibibytes(&self) -> u64 {
        self.bytes / (1024 * 1024 * 1024)
    }
}

impl FromStr for ByteSize {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ValidationError::InvalidByteSize {
                value: s.to_string(),
            });
        }

        // Find where the number ends and the unit begins
        let (num_str, unit) = s
            .find(|c: char| !c.is_ascii_digit())
            .map_or((s, ""), |i| s.split_at(i));

        let num: u64 = num_str
            .parse()
            .map_err(|_| ValidationError::InvalidByteSize {
                value: s.to_string(),
            })?;

        let multiplier: u64 = match unit {
            "" | "B" => 1,
            "Ki" | "KiB" => 1024,
            "Mi" | "MiB" => 1024 * 1024,
            "Gi" | "GiB" => 1024 * 1024 * 1024,
            "Ti" | "TiB" => 1024 * 1024 * 1024 * 1024,
            "K" | "KB" => 1000,
            "M" | "MB" => 1000 * 1000,
            "G" | "GB" => 1000 * 1000 * 1000,
            "T" | "TB" => 1000 * 1000 * 1000 * 1000,
            _ => {
                return Err(ValidationError::InvalidByteSize {
                    value: s.to_string(),
                });
            }
        };

        let bytes =
            num.checked_mul(multiplier)
                .ok_or_else(|| ValidationError::InvalidByteSize {
                    value: s.to_string(),
                })?;

        Ok(Self { bytes })
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const GI: u64 = 1024 * 1024 * 1024;
        const MI: u64 = 1024 * 1024;
        const KI: u64 = 1024;

        if self.bytes >= GI && self.bytes.is_multiple_of(GI) {
            write!(f, "{}Gi", self.bytes / GI)
        } else if self.bytes >= MI && self.bytes.is_multiple_of(MI) {
            write!(f, "{}Mi", self.bytes / MI)
        } else if self.bytes >= KI && self.bytes.is_multiple_of(KI) {
            write!(f, "{}Ki", self.bytes / KI)
        } else {
            write!(f, "{}B", self.bytes)
        }
    }
}

impl Serialize for ByteSize {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Resource limits for a service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resources {
    /// Memory limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<ByteSize>,

    /// CPU cores (fractional allowed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,
}

/// Replica configuration for scaling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Replicas {
    /// Minimum number of replicas.
    #[serde(default = "default_min_replicas")]
    pub min: u32,

    /// Maximum number of replicas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,

    /// CPU threshold percentage for autoscaling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_threshold: Option<u32>,
}

fn default_min_replicas() -> u32 {
    1
}

impl Default for Replicas {
    fn default() -> Self {
        Self {
            min: 1,
            max: None,
            cpu_threshold: None,
        }
    }
}

/// Restart policy for a service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestartPolicy {
    /// The restart policy type.
    #[serde(default)]
    pub policy: RestartPolicyType,

    /// Maximum restart attempts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,

    /// Backoff duration between restarts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backoff: Option<Duration>,
}

/// Types of restart policies.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicyType {
    /// Always restart the service.
    #[default]
    Always,
    /// Only restart on failure.
    OnFailure,
    /// Never restart automatically.
    Never,
}

/// Arbitrary key-value labels.
pub type Labels = HashMap<String, String>;

/// HTTP method for health checks.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// HTTP GET method.
    #[default]
    Get,
    /// HTTP POST method.
    Post,
    /// HTTP HEAD method.
    Head,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_parsing() {
        assert_eq!(Duration::from_str("30s").map(|d| d.as_secs()), Ok(30));
        assert_eq!(Duration::from_str("5m").map(|d| d.as_secs()), Ok(300));
        assert_eq!(Duration::from_str("1h").map(|d| d.as_secs()), Ok(3_600));
        assert_eq!(Duration::from_str("2d").map(|d| d.as_secs()), Ok(172_800));
        assert!(Duration::from_str("invalid").is_err());
    }

    #[test]
    fn test_duration_display() {
        assert_eq!(Duration::from_secs(30).to_string(), "30s");
        assert_eq!(Duration::from_secs(300).to_string(), "5m");
        assert_eq!(Duration::from_secs(3_600).to_string(), "1h");
        assert_eq!(Duration::from_secs(86_400).to_string(), "1d");
    }

    #[test]
    fn test_byte_size_parsing() {
        const MI: u64 = 1024 * 1024;
        const GI: u64 = 1024 * 1024 * 1024;

        assert_eq!(
            ByteSize::from_str("256Mi").map(|b| b.as_bytes()),
            Ok(256 * MI)
        );
        assert_eq!(ByteSize::from_str("1Gi").map(|b| b.as_bytes()), Ok(GI));
        assert_eq!(
            ByteSize::from_str("512Ki").map(|b| b.as_bytes()),
            Ok(512 * 1024)
        );
        assert!(ByteSize::from_str("invalid").is_err());
    }

    #[test]
    fn test_byte_size_display() {
        const MI: u64 = 1024 * 1024;
        const GI: u64 = 1024 * 1024 * 1024;

        assert_eq!(ByteSize::from_bytes(256 * MI).to_string(), "256Mi");
        assert_eq!(ByteSize::from_bytes(GI).to_string(), "1Gi");
    }
}
