use crate::AppContext;
use crate::git::get_branch_name;

/// Perform the action of packing changelog entries for a release
pub(crate) fn cl_pack(ctx: AppContext, manual_channel: Option<&str>) -> anyhow::Result<()> {
    let branch = get_branch_name(&ctx);
    let channel = branch.as_ref().map(|b| b.parse_channel(&ctx)).transpose()?.flatten();
    let version = branch.as_ref().map(|b| b.parse_version(&ctx)).transpose()?.flatten();

    eprintln!(
        "Branch name: {:?}, channel: {:?}, version: {:?}",
        branch, channel, version
    );

    todo!();
}
