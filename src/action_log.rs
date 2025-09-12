use crate::AppContext;
use crate::git::get_branch_name;

pub(crate) fn cl_log(ctx: AppContext) {
    let branch = get_branch_name(&ctx);
    let issue = branch.as_ref().map(|b| b.parse_issue(&ctx)).flatten();

    eprintln!("Branch name: {:?}, issue: {:?}", branch, issue);

    todo!();
}
