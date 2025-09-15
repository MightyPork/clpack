use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// e.g. default, stable, eap
pub type ChannelName = String;

/// e.g. 1.2.3
pub type VersionName = String;

/// e.g. SW-1234-stuff-is-broken (without .md)
pub type EntryName = String;

/// Main app configuration file
#[derive(Debug, Serialize, Deserialize, SmartDefault)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Folder for data files - the tool will manage contents of this folder.
    /// Changelog entries are simple text files that may be edited manually
    /// if corrections need to be made.
    #[default = "changelog"]
    pub data_folder: String,

    /// ID of the default channel - this only matters inside this config file
    #[default = "default"]
    pub default_channel: String,

    /// Path or file name of the default changelog file, relative to the root of the project.
    ///
    /// The name is used as-is.
    #[default = "CHANGELOG.md"]
    pub changelog_file_default: String,

    /// Path or file of a channel-specific changelog file, relative to the root of the project.
    ///
    /// Placeholders supported are:
    /// - `{channel}`, `{Channel}`, `{CHANNEL}` - Channel ID in the respective capitalization
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
    ///
    /// Users may also specify a custom section name.
    ///
    /// Changelog entries under each section will be grouped in the packed changelog.
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
    #[default(Some(r"/^rel\/(\d+\.\d+)$/".to_string()))]
    pub branch_version_pattern: Option<String>,
}
