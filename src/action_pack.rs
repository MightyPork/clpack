use crate::AppContext;
use crate::config::ChannelName;
use crate::git::{BranchName, get_branch_name};
use crate::integrations::youtrack::{
    youtrack_integration_enabled, youtrack_integration_on_release,
};
use crate::store::{Release, Store};
use anyhow::bail;
use colored::Colorize;

pub fn pack_resolve_and_show_preview(
    ctx: &AppContext,
    user_chosen_channel: Option<ChannelName>,
    branch: Option<&BranchName>,
) -> anyhow::Result<Option<(Release, ChannelName)>> {
    let channel = resolve_channel(&ctx, user_chosen_channel, branch)?;
    let store = Store::new(&ctx, false)?;

    let unreleased = store.find_unreleased_changes(&channel)?;

    if unreleased.is_empty() {
        eprintln!("No unreleased changes.");
        return Ok(None);
    }

    println!();
    println!("Changes waiting for release:");
    for entry in &unreleased {
        println!("+ {}", entry.cyan());
    }
    println!();

    let release = Release {
        version: "Unreleased".to_string(),
        entries: unreleased,
    };

    let rendered = store.render_release(&release)?;

    println!("\nPreview:\n\n{}", rendered);

    Ok(Some((release, channel)))
}

/// Resolve channel from current branch or other context info, ask if needed
fn resolve_channel(
    ctx: &AppContext,
    user_chosen_channel: Option<ChannelName>,
    branch: Option<&BranchName>,
) -> anyhow::Result<ChannelName> {
    let (channel_detected, channel_explicit) = match user_chosen_channel {
        Some(ch) => (Some(ch), true), // passed via flag already
        None => (
            branch
                .as_ref()
                .map(|b| b.parse_channel(&ctx.config))
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

    Ok(channel)
}

/// Perform the action of packing changelog entries for a release
pub(crate) fn cl_pack(
    ctx: AppContext,
    user_chosen_channel: Option<ChannelName>,
) -> anyhow::Result<()> {
    let branch = get_branch_name(&ctx);
    let Some((mut release, channel)) =
        pack_resolve_and_show_preview(&ctx, user_chosen_channel, branch.as_ref())?
    else {
        // No changes
        return Ok(());
    };

    let mut store = Store::new(&ctx, false)?;

    // If the branch is named rel/3.40, this can extract 3.40.
    // TODO try to get something better from git!
    let version_base = branch
        .as_ref()
        .map(|b| b.parse_version(&ctx.config))
        .transpose()?
        .flatten();

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

    release.version = version.clone();

    if !inquire::Confirm::new("Continue - write to changelog file?")
        .with_default(true)
        .prompt()?
    {
        eprintln!("{}", "Cancelled.".red());
        return Ok(());
    }

    store.create_release(channel.clone(), release.clone())?;

    println!("{}", "Changelog written.".green());

    // YouTrack
    if youtrack_integration_enabled(&ctx.config, &channel) {
        if inquire::Confirm::new("Update released issues in YouTrack?")
            .with_default(true)
            .prompt()?
        {
            youtrack_integration_on_release(&ctx.config, release)?;
            println!("{}", "YouTrack updated.".green());
        } else {
            eprintln!("{}", "YouTrack changes skipped.".yellow());
            return Ok(());
        }
    }

    Ok(())
}
