use crate::action_log::cl_log;
use crate::action_pack::cl_pack;
use clap::builder::NonEmptyStringValueParser;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::exit;

mod git;

mod action_log;
mod action_pack;

#[derive(Debug)]
struct AppContext {
    config: Config,

    root: PathBuf,
}

/// Main app configuration file
#[derive(Debug, Serialize, Deserialize, SmartDefault)]
#[serde(deny_unknown_fields, default)]
struct Config {
    /// Folder for data files - the tool will manage contents of this folder.
    /// Changelog entries are simple text files that may be edited manually
    /// if corrections need to be made.
    #[default = "changelog"]
    data_folder: String,

    /// ID of the default channel - this only matters inside this config file
    #[default = "default"]
    default_channel: String,

    /// Path or file name of the default changelog file, relative to the root of the project.
    ///
    /// The name is used as-is.
    #[default = "CHANGELOG.md"]
    changelog_file_default: String,

    /// Path or file of a channel-specific changelog file, relative to the root of the project.
    ///
    /// Placeholders supported are:
    /// - `{channel}`, `{Channel}`, `{CHANNEL}` - Channel ID in the respective capitalization
    #[default = "CHANGELOG-{CHANNEL}.md"]
    changelog_file_channel: String,

    /// Changelog sections suggested when creating a new entry.
    ///
    /// Users may also specify a custom section name.
    ///
    /// Changelog entries under each section will be grouped in the packed changelog.
    #[default(vec![
        "New features".to_string(),
        "Improvements".to_string(),
        "Fixes".to_string(),
    ])]
    sections: Vec<String>,

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
    #[default(HashMap::from([
        ("default".to_string(), "/^(?:main|master)$/".to_string())
    ]))]
    channels: HashMap<String, String>,

    /// Regex pattern to extract issue number from a branch name.
    /// There should be one capture group that is the number.
    ///
    /// Example: `/^(SW-\d+)-.*$/` or  `/^(\d+)-.*$/`
    ///
    /// If None, no branch identification will be attempted.
    #[default(Some(r"/^((?:SW-)?\d+)-.*/".to_string()))]
    branch_issue_pattern: Option<String>,

    /// Regex pattern to extract release number from a branch name.
    /// There should be one capture group that is the version.
    ///
    /// Example: `/^rel\/(\d+.\d+)$/`
    ///
    /// If None, no branch identification will be attempted.
    ///
    /// TODO attempt to parse version from package.json, composer.json, Cargo.toml and others
    #[default(Some(r"/^rel\/(\d+\.\d+)$/".to_string()))]
    branch_version_pattern: Option<String>,
}

fn main() {
    let args = clap::Command::new("cl")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .subcommand(
            clap::Command::new("pack")
                .visible_alias("release")
                .about("Create a release changelog entry for the current channel")
                .arg(
                    clap::Arg::new("CHANNEL")
                        .help("Channel ID, possible values depend on project config. None for main channel.")
                        .value_parser(NonEmptyStringValueParser::new())
                        .required(false),
                ),
        )
        .subcommand(clap::Command::new("add")
            .visible_alias("log")
            .about("Add a changelog entry on the current branch"))
        .subcommand_required(false)
        .arg(clap::Arg::new("CONFIG")
            .short('c')
            .long("config")
            .value_parser(NonEmptyStringValueParser::new())
            .required(false))
        .after_help(
            "Call with no arguments to create a changelog entry (same as the \"add\" subcommand).",
        )
        .get_matches();

    let specified_config_file = args.get_one::<String>("CONFIG").map(|s| s.as_str());

    let config_file_name: &str = specified_config_file.unwrap_or("clpack.toml");

    eprintln!("Loading configuration from {}", config_file_name);

    let Ok(root) = std::env::current_dir() else {
        eprintln!("Failed to get current directory - is it deleted / inaccessible?");
        exit(1);
    };

    let config_path = if config_file_name.starts_with("/") {
        // It's an absolute path
        PathBuf::from(config_file_name)
    } else {
        root.join(&config_file_name)
    };

    // Load and parse config

    let config: Config = if let Ok(config_file_content) = std::fs::read_to_string(&config_path) {
        match toml::from_str(&config_file_content) {
            Ok(config) => config,
            Err(e) => {
                eprintln!(
                    "Failed to parse config file ({}): {}",
                    config_path.display(),
                    e
                );
                exit(1);
            }
        }
    } else if specified_config_file.is_some() {
        // Failed to load config the user specifically asked for - make it an error
        eprintln!("Failed to load config file at {}", config_path.display());
        exit(1);
    } else {
        Default::default()
    };

    let ctx = AppContext { config, root };

    // eprintln!("AppCtx: {:?}", ctx);

    match args.subcommand() {
        Some(("pack", subargs)) => {
            let manual_channel = subargs.get_one::<String>("CHANNEL");
            cl_pack(ctx, manual_channel.map(String::as_str));
        }
        None | Some(("add", _)) => cl_log(ctx),
        Some((other, _)) => {
            unimplemented!("Subcommand {other} is not implemented yet");
        }
    }
}
