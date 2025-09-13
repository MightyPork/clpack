use crate::config::Config;
use crate::store::Store;
use colored::Colorize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// Args for cl_init()
pub struct ClInit {
    /// name of the binary, detected from argv/system/env at startup - shown in messages
    pub binary_name: String,
    /// Root of the project
    pub root: PathBuf,
    /// Path to the config file to try to read, or to create
    pub config_path: PathBuf,
}

/// Init the changelog system
pub fn cl_init(opts: ClInit) -> anyhow::Result<()> {
    let mut default_config = Config::default();

    if !opts.config_path.exists() {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&opts.config_path)?;

        println!(
            "Creating clpack config file: {}",
            opts.config_path.display()
        );
        file.write_all(toml::to_string_pretty(&default_config)?.as_bytes())?;
    } else {
        println!(
            "Loading existing config file: {}",
            opts.config_path.display()
        );
        let file_text = std::fs::read_to_string(&opts.config_path)?;
        default_config = toml::from_str(&file_text)?;
    }

    let ctx = crate::AppContext {
        binary_name: opts.binary_name,
        config: default_config,
        root: opts.root,
    };
    let _ = Store::new(&ctx, true)?;

    println!("{}", "Changelog initialized.".green());
    Ok(())
}
