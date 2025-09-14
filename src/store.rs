use crate::AppContext;
use crate::config::{ChannelName, EntryName, VersionName};
use anyhow::bail;
use colored::Colorize;
use faccess::PathExt;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::{OpenOptions, read_to_string};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const DIR_ENTRIES: &str = "entries";
const DIR_CHANNELS: &str = "channels";

/// Changelog store struct
pub struct Store<'a> {
    /// App context, including config
    ctx: &'a AppContext,
    /// Path to the changelog directory
    store_path: PathBuf,
    /// Loaded version history for all channels
    versions: HashMap<ChannelName, ChannelReleaseStore>,
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

        let mut store = Self {
            store_path,
            ctx,
            versions: HashMap::new(),
        };

        store.ensure_internal_subdirs_exist()?;
        store.load_versions()?;

        Ok(store)
    }

    /// Build a log entry file path.
    /// This is a file in the entries storage
    fn make_entry_path(&self, filename: &str) -> PathBuf {
        self.store_path
            .join(DIR_ENTRIES)
            .join(format!("{filename}.md"))
    }

    /// Check if a changelog entry exists. Filename is passed without extension.
    /// This only checks within the current epoch as older files are no longer present.
    pub fn entry_exists(&self, name: &str) -> bool {
        let path = self.make_entry_path(name);
        path.exists()
    }

    /// Load release lists for all channels
    fn load_versions(&mut self) -> anyhow::Result<()> {
        let channels_dir = self.store_path.join(DIR_CHANNELS);

        for ch in self.ctx.config.channels.keys() {
            let channel_file = channels_dir.join(format!("{}.json", ch));
            self.versions.insert(
                ch.clone(),
                ChannelReleaseStore::load(channel_file, ch.clone())?,
            );
        }

        Ok(())
    }

    /// Check and create internal subdirs for the clpack system
    pub fn ensure_internal_subdirs_exist(&self) -> anyhow::Result<()> {
        self.ensure_subdir_exists(DIR_ENTRIES)?;
        self.ensure_subdir_exists(DIR_CHANNELS)?;
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
    pub fn create_entry(&self, name: EntryName, content: String) -> anyhow::Result<()> {
        let path = self.make_entry_path(name.as_str());
        let mut file = OpenOptions::new().write(true).create(true).open(&path)?;

        eprintln!("Writing changelog entry to file: {}", path.display());

        file.write_all(content.as_bytes())?;
        Ok(())
    }

    /// Check if a version was already released (on any channel) - prevents the user from making a mistake in version naming
    pub fn version_exists(&self, version: &str) -> bool {
        for v in self.versions.values() {
            if v.version_exists(version) {
                return true;
            }
        }
        false
    }

    /// Find unreleased changelog entries on a channel
    pub fn find_unreleased_changes(&self, channel: &ChannelName) -> anyhow::Result<Vec<EntryName>> {
        let Some(store) = self.versions.get(channel) else {
            bail!("Channel {channel} does not exist.");
        };

        store.find_unreleased_entries(self.store_path.join(DIR_ENTRIES))
    }

    /// Create a release entry, write it to the releases buffer and to the file.
    pub fn create_release(&mut self, channel: ChannelName, release: Release) -> anyhow::Result<()> {
        let Some(store) = self.versions.get_mut(&channel) else {
            bail!("Channel {channel} does not exist.");
        };

        let config = &self.ctx.config;
        let rendered = release.render(self.store_path.join(DIR_ENTRIES), &config.sections)?;

        let changelog_file = self.ctx.root.join(
            if channel == config.default_channel {
                Cow::Borrowed(config.changelog_file_default.as_str())
            } else {
                Cow::Owned(
                    config
                        .changelog_file_channel
                        .replace("{channel}", &channel.to_lowercase())
                        .replace("{CHANNEL}", &channel.to_uppercase())
                        .replace("{Channel}", &ucfirst(&channel)),
                )
            }
            .as_ref(),
        );

        if changelog_file.exists() {
            let changelog_file_content = read_to_string(&changelog_file)?;
            let old_content = changelog_file_content
                .strip_prefix(&config.changelog_header)
                .unwrap_or(&changelog_file_content);

            let mut outfile = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(changelog_file)?;

            outfile.write_all(
                format!("{}{}{}", config.changelog_header, rendered, old_content).as_bytes(),
            )?;
        } else {
            let mut outfile = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(changelog_file)?;

            outfile.write_all(format!("{}{}", config.changelog_header, rendered).as_bytes())?;
        }

        store.add_version(release)?;
        // Write to the changelog file for this channel
        store.write_to_file()?;
        Ok(())
    }
}

/// Uppercase first char of a string
fn ucfirst(input: &str) -> String {
    let mut c = input.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Summary of a release
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Release {
    /// Name of the version
    pub version: VersionName,
    /// List of entries included in this version
    pub entries: Vec<EntryName>,
}

impl Release {
    /// Render the entry into a Markdown fragment, using h2 (##) as the title.
    pub fn render(
        &self,
        entries_dir: impl AsRef<Path>,
        predefined_sections: &[String],
    ) -> anyhow::Result<String> {
        let mut entries_per_section = HashMap::<String, String>::new();
        let entries_dir = entries_dir.as_ref();
        let unnamed = "".to_string();

        for entry in &self.entries {
            let entry_file = entries_dir.join(&format!("{entry}.md"));

            if !entry_file.exists() || !entry_file.readable() {
                bail!(
                    "Changelog entry file missing or not readable: {}",
                    entry_file.display()
                );
            }

            let file = OpenOptions::new().read(true).open(&entry_file)?;
            let reader = BufReader::new(file);

            let mut current_section = unnamed.clone();
            for line in reader.lines() {
                let line = line?;
                if line.trim().starts_with('#') {
                    // It is a section name
                    let section = line.trim_matches(|c| c == '#' || c == ' ');
                    current_section = section.to_string();
                } else {
                    if let Some(buffer) = entries_per_section.get_mut(&current_section) {
                        buffer.push('\n');
                        buffer.push_str(&line);
                    } else {
                        entries_per_section.insert(current_section.clone(), line);
                    }
                }
            }
        }

        let mut reordered_sections = Vec::<(String, String)>::new();

        // First the unlabelled section (this is probably junk, but it was entered by the user, so keep it)
        if let Some(unlabelled) = entries_per_section.remove("") {
            reordered_sections.push(("".to_string(), unlabelled));
        }

        for section_name in [unnamed].iter().chain(predefined_sections.iter()) {
            if let Some(content) = entries_per_section.remove(section_name) {
                reordered_sections.push((section_name.clone(), content));
            }
        }
        // Leftovers (names authors invented when writing changelog)
        for (section_name, content) in entries_per_section {
            reordered_sections.push((section_name, content));
        }

        let mut buffer = String::new();

        for (section_name, content) in reordered_sections {
            if !section_name.is_empty() {
                buffer.push_str(&format!("## {}\n", section_name));
            }
            buffer.push_str(&content);
            buffer.push('\n');
        }

        Ok(buffer)
    }
}

/// List of releases, deserialized from a file
type ReleaseList = Vec<Release>;

/// Versions store for one channel
struct ChannelReleaseStore {
    /// File where the list of versions is stored
    backing_file: PathBuf,
    /// Name of the channel, for error messages
    channel_name: ChannelName,
    /// List of releases, load from the file
    releases: ReleaseList,
}

impl ChannelReleaseStore {
    /// Load from a versions file
    fn load(releases_file: PathBuf, channel_name: ChannelName) -> anyhow::Result<Self> {
        let releases = if !releases_file.exists() {
            // File did not exist yet, create it
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&releases_file)?;
            f.write_all("{}".as_bytes())?;
            Default::default()
        } else {
            let channel_json = read_to_string(&releases_file)?;
            serde_json::from_str::<ReleaseList>(&channel_json)?
        };

        Ok(Self {
            backing_file: releases_file,
            channel_name,
            releases,
        })
    }

    /// Check if a version is included in a release
    fn version_exists(&self, version: &str) -> bool {
        self.releases.iter().any(|rel| rel.version == version)
    }

    /// Add a version to the channel buffer
    /// The release entry, borrowed, is returned for  further use
    fn add_version(&mut self, release: Release) -> anyhow::Result<()> {
        if self.version_exists(&release.version) {
            bail!(
                "Version {} already exists on channel {}",
                release.version,
                self.channel_name
            );
        }
        self.releases.push(release);
        Ok(())
    }

    /// Write the versions list contained in this store into the backing file.
    fn write_to_file(&self) -> anyhow::Result<()> {
        let encoded = serde_json::to_string_pretty(&self.releases)?;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&self.backing_file)?;
        f.write_all(encoded.as_bytes())?;
        Ok(())
    }

    /// Find entries not yet included in this release channel
    fn find_unreleased_entries(
        &self,
        entries_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Vec<EntryName>> {
        let mut found = vec![];

        for entry in entries_dir.as_ref().read_dir()? {
            let entry = entry?;

            let fname_os = entry.file_name();
            let fname = fname_os.into_string().map_err(|_| {
                anyhow::anyhow!("Failed to parse file name: {}", entry.path().display())
            })?;

            if !entry.metadata()?.is_file() || !fname.ends_with(".md") {
                eprintln!(
                    "{}",
                    format!(
                        "Unexpected item in changelog entries dir: {}",
                        entry.path().display()
                    )
                    .yellow()
                );
                continue;
            }

            let basename = fname.strip_suffix(".md").unwrap();

            if !self
                .releases
                .iter()
                .map(|rel| &rel.entries)
                .flatten()
                .any(|entryname| entryname == basename)
            {
                found.push(fname);
            }
        }

        Ok(found)
    }
}
