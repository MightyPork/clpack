use crate::AppContext;
use crate::git::get_branch_name;

pub(crate) fn cl_pack(ctx: AppContext, manual_channel: Option<&str>) {
    let branch = get_branch_name(&ctx);
    let channel = branch.as_ref().map(|b| b.parse_channel(&ctx)).flatten();
    let version = branch.as_ref().map(|b| b.parse_version(&ctx)).flatten();

    eprintln!(
        "Branch name: {:?}, channel: {:?}, version: {:?}",
        branch, channel, version
    );

    todo!();
}
