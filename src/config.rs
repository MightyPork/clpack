use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// e.g. default, stable, eap
pub type ChannelName = String;

/// e.g. 1.2.3
pub type VersionName = String;

/// e.g. SW-1234-stuff-is-broken (without .md)
pub type EntryName = String;

/// Config file with nice comments
pub const CONFIG_FILE_TEMPLATE: &str = include_str!("assets/config_file_template.toml");

/// ENV / dotenv key for the youtrack integration server URL
/// This is only for unit tests
pub const ENV_YOUTRACK_URL: &str = "CLPACK_YOUTRACK_URL";

/// ENV / dotenv key for the youtrack integration API token
pub const ENV_YOUTRACK_TOKEN: &str = "CLPACK_YOUTRACK_TOKEN";

#[cfg(test)]
#[test]
fn test_template_file() {
    // Check 1. that the example config is valid, and 2. that it matches the defaults in the struct
    let parsed: Config = toml::from_str(CONFIG_FILE_TEMPLATE).unwrap();
    let def = Config::default();
    assert_eq!(parsed, def);
}

/// Main app configuration file
#[derive(Debug, Serialize, Deserialize, SmartDefault, PartialEq, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Name / path of the folder managed by clpack
    #[default = "changelog"]
    pub data_folder: String,

    /// ID of the default channel
    #[default = "default"]
    pub default_channel: String,

    /// Path or file name of the default changelog file, relative to project root (CWD)
    #[default = "CHANGELOG.md"]
    pub changelog_file_default: String,

    /// Path or file of a channel-specific changelog file, relative to project root (CWD).
    /// Supports placeholder `{channel}`, `{Channel}`, `{CHANNEL}`
    #[default = "CHANGELOG-{CHANNEL}.md"]
    pub changelog_file_channel: String,

    /// Title of the changelog file, stripped and put back in front
    #[default = "# Changelog\n\n"]
    pub changelog_header: String,

    /// Pattern for release header
    #[default = "[{VERSION}] - {DATE}"]
    pub release_header: String,

    /// Date format (see patterns supported by the Chrono crate: https://docs.rs/chrono/latest/chrono/format/strftime/index.html )
    #[default = "%Y-%m-%d"]
    pub date_format: String,

    /// Changelog sections suggested when creating a new entry.
    /// The order is maintained.
    ///
    /// Users may also specify custom section names when writing the changelog file.
    #[default(vec![
        "Fixes".to_string(),
        "Improvements".to_string(),
        "New features".to_string(),
        "Internal".to_string(),
    ])]
    pub sections: Vec<String>,

    /// Changelog channels - how to identify them from git branch names
    ///
    /// - Key - changelog ID; this can be used in the channel file name. Examples: default, eap, beta
    /// - Value - git branch name to recognize the channel. This is a regex pattern.
    ///
    /// At least one channel must be defined, with the name defined in `default_channel`
    ///
    /// # Value format
    /// For simple branch names without special symbols that do not change, e.g. `main`, `master`, `test`, you can just use the name as is.
    /// To specify a regex, enclose it in slashes, e.g. /rel\/foo/
    ///
    /// If you have a naming schema like e.g. `beta/1.0` where only the prefix stays the same, you may use e.g. `^beta/.*`
    #[default(IndexMap::from([
        ("default".to_string(), "/^(?:main|master)$/".to_string())
    ]))]
    pub channels: IndexMap<ChannelName, String>,

    /// Regex pattern to extract issue number from a branch name.
    /// There should be one capture group that is the number.
    ///
    /// Example: `/^(SW-\d+)-.*$/` or  `/^(\d+)-.*$/`
    ///
    /// If None, no branch identification will be attempted.
    #[default(Some(r"/^((?:SW-)?\d+)-.*/".to_string()))]
    pub branch_issue_pattern: Option<String>,

    /// Regex pattern to extract release number from a branch name.
    /// There should be one capture group that is the version.
    ///
    /// Example: `/^rel\/(\d+.\d+)$/`
    ///
    /// If None, no branch identification will be attempted.
    ///
    /// TODO attempt to parse version from package.json, composer.json, Cargo.toml and others
    #[default(Some(r"/^rel\/([\d.]+)$/".to_string()))]
    pub branch_version_pattern: Option<String>,

    /// Integrations config
    pub integrations: IntegrationsConfig,
}

/// Integrations config
#[derive(Debug, Serialize, Deserialize, SmartDefault, PartialEq, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct IntegrationsConfig {
    /// YouTrack integration
    pub youtrack: YouTrackIntegrationConfig,
}

#[derive(Debug, Serialize, Deserialize, SmartDefault, PartialEq, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct YouTrackIntegrationConfig {
    /// Enable the integration
    pub enabled: bool,

    /// URL of the youtrack server (just https://domain)
    #[default = "https://example.youtrack.cloud"]
    pub url: String,

    /// Channels filter
    #[default(vec![
        "default".to_string(),
    ])]
    pub channels: Vec<ChannelName>,

    /// Name of the State option to switch to when generating changelog (e.g. Released)
    pub released_state: Option<String>,

    /// Name of the version field (Available in version)
    pub version_field: Option<String>,
}
