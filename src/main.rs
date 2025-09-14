use crate::action_init::{ClInit, cl_init};
use crate::action_log::cl_log;
use crate::action_pack::cl_pack;
use crate::config::Config;
use anyhow::bail;
use clap::builder::NonEmptyStringValueParser;
use colored::Colorize;
use std::path::PathBuf;
use std::process::exit;

mod config;

mod git;

mod action_log;
mod action_pack;

mod action_init;

mod store;

#[derive(Debug)]
pub struct AppContext {
    /// Name of the cl binary
    pub binary_name: String,

    /// Config loaded from file or defaults
    pub config: Config,

    /// Root of the project
    pub root: PathBuf,
}

fn main() {
    if let Err(e) = main_try() {
        eprintln!("{}", e.to_string().red().bold());
        exit(1);
    }
}

fn main_try() -> anyhow::Result<()> {
    let binary_name = std::env::current_exe()
        .map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .ok()
        .flatten()
        .unwrap_or_else(|| "cl".to_string());

    let args = clap::Command::new(&binary_name)
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .subcommand(clap::Command::new("init")
            .about("Create the changelog folder and the default config file in the current working directory, if they do not exist yet."))
        .subcommand(
            clap::Command::new("pack")
                .visible_alias("release")
                .about("Pack changelog entries to a changelog section"),
        )
        .subcommand(clap::Command::new("add")
            .visible_alias("log")
            .about("Add a changelog entry on the current branch"))
        .subcommand(clap::Command::new("flush")
            .about("Remove all changelog entries that were already released on all channels - clean up the changelog dir. Use e.g. when making a major release where all channel branches are merged."))
        .subcommand(clap::Command::new("status")
            .about("Show changelog entries currently waiting for release on the current channel"))
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

    // eprintln!("Loading configuration from {}", config_file_name);

    let Ok(root) = std::env::current_dir() else {
        bail!("Failed to get current directory - is it deleted / inaccessible?");
    };

    let config_path = root.join(&config_file_name); // if absolute, it is replaced by it

    if let Some(("init", _)) = args.subcommand() {
        return cl_init(ClInit {
            binary_name,
            root,
            config_path,
        });
    }

    // Load and parse config
    let config: Config = if let Ok(config_file_content) = std::fs::read_to_string(&config_path) {
        match toml::from_str(&config_file_content) {
            Ok(config) => config,
            Err(e) => {
                bail!(
                    "Failed to parse config file ({}): {}",
                    config_path.display(),
                    e
                );
            }
        }
    } else if specified_config_file.is_some() {
        // Failed to load config the user specifically asked for - make it an error
        bail!("Failed to load config file at {}", config_path.display());
    } else {
        Default::default()
    };

    let ctx = AppContext {
        binary_name,
        config,
        root,
    };

    // eprintln!("AppCtx: {:?}", ctx);

    match args.subcommand() {
        Some(("pack", _)) => {
            cl_pack(ctx)?;
        }
        None | Some(("add", _)) => cl_log(ctx)?,
        // TODO: status, flush
        Some((other, _)) => {
            bail!("Subcommand {other} is not implemented yet");
        }
    }

    Ok(())
}
