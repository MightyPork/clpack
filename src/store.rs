use crate::AppContext;
use anyhow::bail;
use faccess::PathExt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub struct Store<'a> {
    ctx: &'a AppContext,

    store_path: PathBuf,
}

impl<'a> Store<'a> {
    pub fn new(ctx: &'a AppContext, init: bool) -> anyhow::Result<Self> {
        let store_path = ctx.root.join(&ctx.config.data_folder);

        if !store_path.is_dir() {
            if init {
                // Try to create it
                eprintln!("Creating changelog dir: {}", store_path.display());
                std::fs::create_dir_all(&store_path)?;
            } else {
                bail!(
                    "Changelog directory does not exist: {}. Use `{} init` to create it.",
                    ctx.binary_name,
                    store_path.display()
                );
            }
        }

        if !store_path.writable() {
            bail!(
                "Changelog directory is not writable: {}",
                store_path.display()
            );
        }

        let store = Self { store_path, ctx };

        store.ensure_internal_subdirs_exist()?;

        Ok(store)
    }

    /// Build a log entry file path.
    /// This is a file in the entries storage
    fn make_entry_path(&self, filename: &str) -> PathBuf {
        self.store_path
            .join("entries")
            .join(format!("{filename}.md"))
    }

    /// Check if a changelog entry exists. Filename is passed without extension.
    /// This only checks within the current epoch as older files are no longer present.
    pub fn entry_exists(&self, name: &str) -> bool {
        let path = self.make_entry_path(name);
        path.exists()
    }

    /// Check and create internal subdirs for the clpack system
    pub fn ensure_internal_subdirs_exist(&self) -> anyhow::Result<()> {
        self.ensure_subdir_exists("entries")?;
        // TODO

        Ok(())
    }

    /// make sure a subdir exists, creating if needed.
    ///
    /// Note there is no lock so there can be a TOCTOU bug later if someone deletes the path - must be checked and handled.
    fn ensure_subdir_exists(&self, name: &str) -> anyhow::Result<()> {
        let subdir = self.store_path.join(name);

        if !subdir.is_dir() {
            if subdir.exists() {
                bail!(
                    "Changelog subdir path is clobbered, must be a writable directory or not exist (will be crated): {}",
                    subdir.display()
                );
            }
            eprintln!("Creating changelog subdir: {}", subdir.display());
            std::fs::create_dir_all(&subdir)?;
        }

        if !subdir.writable() {
            bail!("Changelog subdir is not writable: {}", subdir.display());
        }

        std::fs::File::create(subdir.join(".gitkeep"))?;

        Ok(())
    }

    /// Create a changelog entry file and write content to it
    pub fn create_entry(&self, name: String, content: String) -> anyhow::Result<()> {
        let path = self.make_entry_path(name.as_str());
        let mut file = OpenOptions::new().write(true).create(true).open(&path)?;

        eprintln!("Writing to file: {}", path.display());

        file.write_all(content.as_bytes())?;
        Ok(())
    }
}
