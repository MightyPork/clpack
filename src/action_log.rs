use crate::AppContext;
use crate::git::BranchOpt;
use crate::git::get_branch_name;
use crate::store::Store;
use anyhow::bail;
use colored::Colorize;

/// Perform the action of adding a new log entry
pub(crate) fn cl_log(ctx: AppContext) -> anyhow::Result<()> {
    let store = Store::new(&ctx, false)?;
    store.ensure_internal_subdirs_exist()?;

    let branch = get_branch_name(&ctx);
    let issue = branch
        .as_ref()
        .map(|b| b.parse_issue(&ctx))
        .transpose()?
        .flatten();

    if let Some(num) = &issue {
        println!("{}", format!("Issue # parsed from branch: {num}").green());
    } else {
        eprintln!(
            "{}",
            format!(
                "Issue not recognized from branch name! (\"{}\")",
                branch.as_str_or_default()
            )
            .yellow()
        );
    }

    let mut entry_name = branch.as_str_or_default().to_string();

    // Space
    println!();

    loop {
        // Ask for filename
        let mut query = inquire::Text::new("Log entry name:")
            .with_help_message("Used as a filename, without extension");
        if issue.is_some() {
            query = query.with_initial_value(&entry_name);
        }
        entry_name = query.prompt()?;

        if entry_name.is_empty() {
            bail!("Cancelled");
        }

        if store.entry_exists(&entry_name) {
            println!("{}", "Entry already exists, try different name.".red());
        } else {
            break;
        }
    }

    // Space
    println!();

    // Ask for sections
    let sections = inquire::MultiSelect::new(
        "Choose changelog sections to pre-generate (at least one)",
        ctx.config.sections.clone(),
    )
    .prompt()?;

    if sections.is_empty() {
        bail!("Cancelled");
    }

    let mut prefill_text = String::new();

    for section in sections {
        if !prefill_text.is_empty() {
            prefill_text.push('\n');
        }
        prefill_text.push_str(&format!("# {section}\n"));
        if let Some(num) = &issue {
            prefill_text.push_str(&format!("-  (#{num})\n"));
        } else {
            prefill_text.push_str("- \n");
        }
    }

    println!(
        "\nPreview of changelog entry \"{entry_name}\" (not yet saved)\n\n{}\n",
        prefill_text
    );

    // Edit the file
    let mut text = inquire::Editor::new("Edit as needed, then confirm")
        .with_predefined_text(&prefill_text)
        .with_file_extension("md")
        .prompt()?;

    if text.is_empty() {
        text = prefill_text;
    }

    if !text.ends_with('\n') {
        text.push('\n');
    }

    store.create_entry(entry_name, text)?;

    println!("{}", "Done.".green());
    Ok(())
}
