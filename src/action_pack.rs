use crate::AppContext;
use crate::config::ChannelName;
use crate::git::get_branch_name;
use crate::store::{Release, Store};
use anyhow::bail;
use colored::Colorize;

/// Perform the action of packing changelog entries for a release
pub(crate) fn cl_pack(ctx: AppContext, channel: Option<ChannelName>) -> anyhow::Result<()> {
    let mut store = Store::new(&ctx, false)?;
    let branch = get_branch_name(&ctx);

    let (channel_detected, channel_explicit) = match channel {
        Some(ch) => (Some(ch), true), // passed via flag already
        None => (
            branch
                .as_ref()
                .map(|b| b.parse_channel(&ctx))
                .transpose()?
                .flatten(),
            false,
        ),
    };

    if let Some(ch) = &channel_detected
        && !ctx.config.channels.contains_key(ch)
    {
        bail!("No such channel: {ch}");
    }

    // If the branch is named rel/3.40, this can extract 3.40.
    // TODO try to get something better from git!
    let version_base = branch
        .as_ref()
        .map(|b| b.parse_version(&ctx))
        .transpose()?
        .flatten();

    // TODO detect version from git query?

    // TODO remove this
    eprintln!(
        "Branch name: {:?}, channel: {:?}, version: {:?}",
        branch, channel_detected, version_base
    );

    // Ask for the channel
    let channel = if ctx.config.channels.len() > 1 {
        if channel_explicit {
            channel_detected.unwrap()
        } else {
            let channels = ctx.config.channels.keys().collect::<Vec<_>>();
            let mut starting_index = None;
            if let Some(channel) = channel_detected {
                starting_index = channels.iter().position(|ch| *ch == &channel);
            }
            let mut query = inquire::Select::new("Release channel?", channels);
            if let Some(index) = starting_index {
                query = query.with_starting_cursor(index);
            }
            query.prompt()?.to_string()
        }
    } else {
        // Just one channel, so use that
        ctx.config.default_channel.clone()
    };
    println!("Channel: {}", channel.green().bold());

    let unreleased = store.find_unreleased_changes(&channel)?;

    if unreleased.is_empty() {
        eprintln!("No unreleased changes.");
        return Ok(());
    }

    println!();
    println!("Changes waiting for release:");
    for entry in &unreleased {
        println!("+ {}", entry.cyan());
    }
    println!();

    // Ask for the version
    let mut version = version_base.unwrap_or_default();
    loop {
        // Ask for full version
        version = inquire::Text::new("Version:")
            .with_initial_value(&version)
            .prompt()?;

        if version.is_empty() {
            bail!("Cancelled");
        }

        if store.version_exists(&version) {
            println!("{}", "Version already exists, try again or cancel.".red());
        } else {
            break;
        }
    }

    let release = Release {
        version,
        entries: unreleased,
    };

    let rendered = store.render_release(&release)?;

    println!("\n\nPreview:\n\n{}\n", rendered);

    if !inquire::Confirm::new("Continue - write to changelog file?")
        .with_default(true)
        .prompt()?
    {
        eprintln!("{}", "Cancelled.".red());
        return Ok(());
    }

    store.create_release(channel, release)?;

    println!("{}", "Changelog written.".green());
    Ok(())
}
