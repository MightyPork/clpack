use crate::AppContext;
use crate::action_pack::pack_resolve_and_show_preview;
use crate::config::ChannelName;
use crate::git::{get_branch_name};

/// Perform the action of packing changelog entries for a release
pub(crate) fn cl_status(
    ctx: AppContext,
    user_chosen_channel: Option<ChannelName>,
) -> anyhow::Result<()> {
    let branch = get_branch_name(&ctx);
    pack_resolve_and_show_preview(&ctx, user_chosen_channel, branch.as_ref())?;
    Ok(())
}
