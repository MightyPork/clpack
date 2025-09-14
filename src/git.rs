use crate::AppContext;
use anyhow::bail;
use std::fmt::Display;
use std::fmt::Formatter;

#[derive(Debug, Clone)]
pub struct BranchName(pub String);

impl Display for BranchName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Best effort identify git branch by looking into the .git folder
pub fn get_branch_name(ctx: &AppContext) -> Option<BranchName> {
    let path_to_head = ctx.root.join(".git").join("HEAD");

    if !path_to_head.is_file() {
        return None;
    }

    let contents = std::fs::read_to_string(path_to_head).ok()?;

    if let Some(branch) = contents.strip_prefix("ref: refs/heads/") {
        let b = branch.trim();
        if b.is_empty() {
            None
        } else {
            Some(BranchName(branch.trim().to_owned()))
        }
    } else {
        None
    }
}

impl BranchName {
    /// Extract a value from a branch name using a regex given as string.
    ///
    /// pat_s - the regex pattern
    /// regex_conf_name - name of the conf field the regex came from, for error messages
    fn parse_using_regex(
        &self,
        template: &str,
        regex_conf_name: &str,
    ) -> anyhow::Result<Option<String>> {
        let Some(pat_s) = as_regex_pattern(template) else {
            bail!(
                "Config field \"{regex_conf_name}\" must contain a regex (encased in slashes). Found: {template}"
            );
        };

        let pat = match regex::Regex::new(pat_s) {
            Ok(pat) => pat,
            Err(e) => {
                bail!("Invalid regex in \"{regex_conf_name}\": {pat_s}\nError: {e}");
            }
        };

        let num_captures = pat.captures_len();
        if num_captures != 2 {
            // capture "0" is the whole matched string
            bail!(
                "The pattern \"{regex_conf_name}\" is not applicable: {pat_s}\nThere must be exactly one capturing group. Found {}",
                num_captures - 1
            );
        }

        let Some(matches) = pat.captures(&self.0) else {
            return Ok(None);
        };
        let Some(amatch) = matches.get(1) else {
            return Ok(None);
        };
        Ok(Some(amatch.as_str().to_owned()))
    }

    /// Parse version from this branch name.
    ///
    /// Aborts if the configured regex pattern is invalid.
    pub fn parse_version(&self, ctx: &AppContext) -> anyhow::Result<Option<String>> {
        let Some(pat) = ctx.config.branch_version_pattern.as_ref() else {
            return Ok(None);
        };
        self.parse_using_regex(pat, "branch_version_pattern")
    }

    /// Parse issue number from this branch name.
    ///
    /// Aborts if the configured regex pattern is invalid.
    pub fn parse_issue(&self, ctx: &AppContext) -> anyhow::Result<Option<String>> {
        let Some(pat) = ctx.config.branch_issue_pattern.as_ref() else {
            return Ok(None);
        };
        self.parse_using_regex(pat, "branch_issue_pattern")
    }

    /// Try to detect a release channel from this branch name (e.g. stable, EAP)
    pub fn parse_channel(&self, ctx: &AppContext) -> anyhow::Result<Option<String>> {
        for (channel_id, template) in &ctx.config.channels {
            if template.is_empty() {
                // Channel only for manual choosing
                continue;
            }
            if let Some(pat_s) = as_regex_pattern(template) {
                let pat = match regex::Regex::new(pat_s) {
                    Ok(pat) => pat,
                    Err(e) => {
                        bail!("Invalid regex for channel \"{channel_id}\": {template}\nError: {e}");
                    }
                };

                if pat.is_match(&self.0) {
                    return Ok(Some(channel_id.to_owned()));
                }
            } else {
                // No regex - match it verbatim
                if &self.0 == template {
                    return Ok(Some(channel_id.to_owned()));
                } else {
                    continue;
                }
            }
        }
        Ok(None)
    }
}

/// If the string is encased in slashes, return the inner part. Otherwise, return None.
fn as_regex_pattern(input: &str) -> Option<&str> {
    input.strip_prefix('/')?.strip_suffix('/')
}

pub trait BranchOpt {
    fn as_str_or_default(&self) -> &str;
}

impl BranchOpt for Option<BranchName> {
    fn as_str_or_default(&self) -> &str {
        self.as_ref().map(|b| b.0.as_str()).unwrap_or_default()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_as_regex_pattern() {
        assert_eq!(as_regex_pattern("foo"), None);
        assert_eq!(as_regex_pattern("/foo"), None);
        assert_eq!(as_regex_pattern("foo/"), None);
        assert_eq!(as_regex_pattern("/foo/"), Some("foo"));
    }

    #[test]
    fn test_parse_version() {
        let ctx = AppContext {
            binary_name: "cl".to_string(),
            config: Default::default(),
            root: PathBuf::from("/tmp/"), // will not be used
        };

        assert_eq!(
            BranchName("rel/3.14".to_string())
                .parse_version(&ctx)
                .unwrap(),
            Some("3.14".to_string())
        );

        assert_eq!(
            BranchName("rel/foo".to_string())
                .parse_version(&ctx)
                .unwrap(),
            None
        );
    }

    #[test]
    fn test_parse_issue() {
        let ctx = AppContext {
            binary_name: "cl".to_string(),
            config: Default::default(),
            root: PathBuf::from("/tmp/"), // will not be used
        };

        assert_eq!(
            BranchName("1234-bober-kurwa".to_string())
                .parse_issue(&ctx)
                .unwrap(),
            Some("1234".to_string())
        );

        assert_eq!(
            BranchName("SW-778-jakie-bydłe-jebane".to_string())
                .parse_issue(&ctx)
                .unwrap(),
            Some("SW-778".to_string())
        );

        assert_eq!(
            BranchName("nie-spierdalaj-mordo".to_string())
                .parse_issue(&ctx)
                .unwrap(),
            None
        );
    }

    #[test]
    fn test_parse_channel() {
        let ctx = AppContext {
            binary_name: "cl".to_string(),
            config: Default::default(),
            root: PathBuf::from("/tmp/"), // will not be used
        };

        assert_eq!(
            BranchName("main".to_string()).parse_channel(&ctx).unwrap(),
            Some("default".to_string())
        );

        assert_eq!(
            BranchName("master".to_string())
                .parse_channel(&ctx)
                .unwrap(),
            Some("default".to_string())
        );

        assert_eq!(
            BranchName("my-cool-feature".to_string())
                .parse_version(&ctx)
                .unwrap(),
            None
        );
    }
}
