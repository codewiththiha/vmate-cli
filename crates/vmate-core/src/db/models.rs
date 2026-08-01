//! Data models mirroring the `configs` table.

use crate::country::CountryCode;
use chrono::{DateTime, Utc};

/// A row from the `configs` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredConfig {
    pub id: i64,
    pub path: String,
    pub path_sha256: String,
    pub remote_host: Option<String>,
    pub country: String,
    pub country_source: String,
    #[sqlx(try_from = "String")]
    pub status: ConfigStatus,
    pub success_count: i64,
    pub failure_count: i64,
    pub skipped_count: i64,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_skipped_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl StoredConfig {
    /// The country code as a validated [`CountryCode`].
    pub fn country_code(&self) -> CountryCode {
        CountryCode::new(&self.country).unwrap_or_else(|_| CountryCode::unknown())
    }
}

/// The current connectivity status of a stored config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigStatus {
    Unknown,
    Success,
    Failed,
}

impl ConfigStatus {
    /// The value stored in the `status` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigStatus::Unknown => "unknown",
            ConfigStatus::Success => "success",
            ConfigStatus::Failed => "failed",
        }
    }

    /// Parse a value from the database.
    pub fn from_db(s: &str) -> Self {
        match s {
            "success" => ConfigStatus::Success,
            "failed" => ConfigStatus::Failed,
            _ => ConfigStatus::Unknown,
        }
    }
}

impl TryFrom<String> for ConfigStatus {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(ConfigStatus::from_db(&value))
    }
}

impl std::fmt::Display for ConfigStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a config's country was determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CountrySource {
    Unknown,
    FileName,
    RemoteHost,
    IpApi,
    Import,
}

impl CountrySource {
    /// The value stored in the `country_source` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            CountrySource::Unknown => "unknown",
            CountrySource::FileName => "filename",
            CountrySource::RemoteHost => "remote_host",
            CountrySource::IpApi => "ip_api",
            CountrySource::Import => "import",
        }
    }

    /// Parse a value from the database.
    pub fn from_db(s: &str) -> Self {
        match s {
            "filename" => CountrySource::FileName,
            "remote_host" => CountrySource::RemoteHost,
            "ip_api" => CountrySource::IpApi,
            "import" => CountrySource::Import,
            _ => CountrySource::Unknown,
        }
    }
}
