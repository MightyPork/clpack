//! Youtrack integration (mark issues as Released when packing to changelog, change Available in version)

use crate::config::{ChannelName, ENV_YOUTRACK_TOKEN, ENV_YOUTRACK_URL, VersionName};
use crate::git::BranchName;
use crate::store::Release;
use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// ID of a youtrack project
type ProjectId = String;

pub fn youtrack_integration_enabled(config: &crate::Config, channel: &ChannelName) -> bool {
    let ytconf = &config.integrations.youtrack;
    ytconf.enabled
        // Channel filter
        && ytconf.channels.contains(&channel)
        // URL is required
        && (!ytconf.url.is_empty() || dotenv::var(ENV_YOUTRACK_URL).is_ok_and(|v| !v.is_empty()))
        // Token is required
        && dotenv::var(ENV_YOUTRACK_TOKEN).is_ok_and(|v| !v.is_empty())
        // Check if we have something to do
        && (ytconf.version_field.as_ref().is_some_and(|v| !v.is_empty())
            || ytconf
                .released_state
                .as_ref()
                .is_some_and(|v| !v.is_empty()))
}

pub fn youtrack_integration_on_release(
    config: &crate::Config,
    release: Release,
) -> anyhow::Result<()> {
    let ytconf = &config.integrations.youtrack;
    let url = dotenv::var(ENV_YOUTRACK_URL)
        .ok()
        .unwrap_or_else(|| ytconf.url.clone());

    if url.is_empty() {
        bail!("YouTrack URL is empty!");
    }
    let token = dotenv::var(ENV_YOUTRACK_TOKEN).context("Error getting YouTrack token")?;

    if token.is_empty() {
        bail!("YouTrack token is empty!");
    }

    let client = YouTrackClient::new(url, &token)?;

    let mut project_id_opt = None;
    let mut set_version_opt = None;

    let prefixed_version = format!("{}{}", ytconf.version_prefix, release.version);

    let date = chrono::Utc::now();
    for entry in release.entries {
        let branch_name = BranchName(entry);
        let Ok(Some(issue_num)) = branch_name.parse_issue(config) else {
            eprintln!("No issue number recognized in {}", branch_name.0);
            continue;
        };

        // Assume all tickets belong to the same project

        if project_id_opt.is_none() {
            match client.find_project_id(&issue_num) {
                Ok(project_id) => {
                    project_id_opt = Some(project_id);
                }
                Err(e) => {
                    eprintln!("Failed to find project number from {issue_num}: {e}");
                    continue;
                }
            }
        }

        let project_id = project_id_opt.as_ref().unwrap(); // We know it is set now

        if let Some(field) = &ytconf.version_field
            && set_version_opt.is_none()
        {
            let set_version = SetVersion {
                field_name: field,
                version: &prefixed_version,
            };

            client.ensure_version_exists_in_project(&project_id, &set_version, Some(date))?;

            set_version_opt = Some(set_version);
        }

        println!("Update issue {issue_num} ({}) in YouTrack", branch_name.0);
        client.set_issue_version_and_state_by_name(
            &issue_num,
            set_version_opt.as_ref(),
            ytconf.released_state.as_deref(),
        )?;
    }

    Ok(())
}

/// YouTrack API client (with only the bare minimum of the API implemented to satisfy clpack's needs)
pub struct YouTrackClient {
    /// HTTPS client with default presets to access the API
    client: reqwest::blocking::Client,
    /// Base URL of the API server
    url: String,
}

/// Error received from the API instead of the normal response
#[derive(Deserialize)]
struct YoutrackErrorResponse {
    /// Error ID
    error: String,
    /// Error message
    error_description: String,
}

impl YouTrackClient {
    /// Create a YouTrack client
    ///
    /// url - API server base URL (e.g. https://mycompany.youtrack.cloud)
    /// token - JWT-like token, starts with "perm-". Obtained from YouTrack profile settings
    pub fn new(url: impl ToString, token: &str) -> anyhow::Result<Self> {
        let token_bearer = format!("Bearer {token}"); // 🐻

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_str(&token_bearer)?);
        headers.insert("Content-Type", HeaderValue::from_str("application/json")?);
        headers.insert("Accept", HeaderValue::from_str("application/json")?);

        Ok(YouTrackClient {
            url: url.to_string(),
            client: reqwest::blocking::Client::builder()
                .default_headers(headers)
                .build()?,
        })
    }

    fn parse_youtrack_error_response(payload: &str) -> anyhow::Error {
        if let Ok(e) = serde_json::from_str::<YoutrackErrorResponse>(&payload) {
            anyhow::format_err!("Error from YouTrack: {} - {}", e.error, e.error_description)
        } else {
            anyhow::format_err!("Error from YouTrack (unknown response format): {payload}")
        }
    }

    /// Send a GET request with query parameters. Deserialize response.
    fn get_json<T: Serialize + ?Sized, O: DeserializeOwned>(
        &self,
        api_path: String,
        query: &T,
    ) -> anyhow::Result<O> {
        let url = format!(
            "{base}/api/{path}",
            base = self.url.trim_end_matches('/'),
            path = api_path.trim_start_matches('/')
        );

        debug!("GET {}", url);

        let response = self.client.get(&url).query(query).send()?;
        let is_ok = response.status().is_success();
        let response_text = response.text()?;

        debug!("Resp = {}", response_text);

        if !is_ok {
            return Err(Self::parse_youtrack_error_response(&response_text));
        }

        Ok(serde_json::from_str(&response_text)?)
    }

    /// Send a POST request with query parameters and serializable (JSON) body. Deserialize response.
    fn post_json<T: Serialize + ?Sized, B: Serialize + ?Sized, O: DeserializeOwned>(
        &self,
        api_path: String,
        body: &B,
        query: &T,
    ) -> anyhow::Result<O> {
        let url = format!(
            "{base}/api/{path}",
            base = self.url.trim_end_matches('/'),
            path = api_path.trim_start_matches('/')
        );

        debug!("POST {}", url);

        let body_serialized = serde_json::to_string(body)?;
        let response = self
            .client
            .post(&url)
            .query(query)
            .body(body_serialized.into_bytes())
            .send()?;

        let is_ok = response.status().is_success();
        let response_text = response.text()?;

        debug!("Resp = {}", response_text);

        if !is_ok {
            return Err(Self::parse_youtrack_error_response(&response_text));
        }

        Ok(serde_json::from_str(&response_text)?)
    }

    /// Find YouTrack project ID from an issue name
    pub fn find_project_id(&self, issue_name: &str) -> anyhow::Result<ProjectId> {
        #[derive(Deserialize)]
        struct Issue {
            project: Project,
        }

        #[derive(Deserialize)]
        struct Project {
            id: ProjectId,
        }

        let issue: Issue =
            self.get_json(format!("issues/{issue_name}"), &[("fields", "project(id)")])?;

        // example:
        // {"project":{"id":"0-172","$type":"Project"},"$type":"Issue"}

        Ok(issue.project.id)
    }

    /// Try to find a version by name in a YouTrack project.
    /// If it is not found but we find the field where to add it, the version will be created.
    ///
    /// - project_id - obtained by `find_project_id()`
    /// - version_info - version name and field name
    /// - release_date - if creating, a date YYYY-MM-DD can be passed here. It will be stored into the
    ///   newly created version & it will be marked as released.
    pub fn ensure_version_exists_in_project(
        &self,
        project_id: &str,
        version_info: &SetVersion,
        release_date: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        type BundleID = String;
        type FieldID = String;

        #[derive(Deserialize)]
        struct BudleDescription {
            id: BundleID,
        }

        #[derive(Deserialize)]
        struct FieldDescription {
            name: String,
            id: FieldID,
        }

        #[derive(Deserialize)]
        struct YTCustomField {
            // Bundle is sometimes missing - we skip these entries
            bundle: Option<BudleDescription>,
            field: FieldDescription,
        }

        // Find field description
        let fields: Vec<YTCustomField> = self.get_json(
            format!("admin/projects/{project_id}/customFields"),
            &[("fields", "field(name,id),bundle(id)"), ("top", "200")],
        )?;

        // Find the field we want in the list (XXX this can probably be done with some API query?)
        let mut field_bundle = None;
        for entry in fields {
            if &entry.field.name == version_info.field_name
                && let Some(bundle) = entry.bundle
            {
                field_bundle = Some((entry.field.id, bundle.id));
                break;
            }
        }

        // Got something?
        let Some((_field_id, bundle_id)) = field_bundle else {
            bail!(
                "YouTrack version field {field_name} not found in the project {project_id}",
                field_name = version_info.field_name
            );
        };

        println!("Found YouTrack version field, checking defined versions");

        #[derive(Deserialize)]
        struct YTVersion {
            name: VersionName,
            #[allow(unused)]
            id: String,
        }

        // Look at options already defined on the field
        let versions: Vec<YTVersion> = self.get_json(
            format!("admin/customFieldSettings/bundles/version/{bundle_id}/values"),
            &[("fields", "id,name"), ("top", "500")],
        )?;

        // Is our version defined?
        if versions.iter().any(|v| v.name == version_info.version) {
            eprintln!(
                "Version {v} already exists in YouTrack",
                v = version_info.version
            );
            return Ok(());
        }

        println!(
            "Creating version in YouTrack: {v}",
            v = version_info.version
        );

        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct CreateVersionBody {
            name: String,
            archived: bool,
            released: bool,
            releaseDate: Option<i64>,
            // archived
        }

        let request_body = CreateVersionBody {
            name: version_info.version.to_string(),
            archived: false,
            released: release_date.is_some(),
            releaseDate: release_date.map(|d| d.timestamp()),
        };

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        struct CreateVersionResponse {
            releaseDate: Option<i64>,
            released: bool,
            archived: bool,
            name: String,
            id: String,
        }

        let resp: CreateVersionResponse = self.post_json(
            format!("admin/customFieldSettings/bundles/version/{bundle_id}/values"),
            &request_body,
            &[("fields", "id,name,released,releaseDate,archived")],
        )?;

        // Example response:
        // {"releaseDate":null,"released":false,"archived":false,"name":"TEST1","id":"232-356","$type":"VersionBundleElement"}
        // {"releaseDate":1758619201,"released":true,"archived":false,"name":"TEST2","id":"232-358","$type":"VersionBundleElement"}

        debug!("Created version entry = {:#?}", resp);
        println!("Version {v} created in YouTrack.", v = version_info.version);

        Ok(())
    }

    /// Modify a YouTrack issue by changing its State and setting "Available in version".
    ///
    /// Before calling this, make sure the version exists, e.g. using `ensure_version_exists_in_project`
    ///
    /// - issue_id - e.g. SW-1234
    /// - version_field_name - name of the YT custom field to modify
    /// - version_name - name of the version, e.g. 1.0.0
    /// - target_state_name name of the State to switch the issue to (None for no-op)
    pub fn set_issue_version_and_state_by_name(
        &self,
        issue_id: &str,
        version: Option<&SetVersion>,
        state: Option<&str>,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct PatchIssueBody {
            customFields: Vec<CustomFieldValue>,
        }

        #[derive(Serialize)]
        struct EnumValue {
            name: String,
        }

        #[derive(Serialize)]
        struct CustomFieldValue {
            name: String,
            #[serde(rename = "$type")]
            field_type: String,
            value: EnumValue,
        }

        let mut custom_fields = Vec::new();

        if let Some(version) = version {
            custom_fields.push(CustomFieldValue {
                name: version.field_name.to_string(),
                field_type: "SingleVersionIssueCustomField".to_string(),
                value: EnumValue {
                    name: version.version.to_string(),
                },
            });
        }

        if let Some(target_state_name) = state {
            custom_fields.push(CustomFieldValue {
                name: "State".to_string(),
                field_type: "StateIssueCustomField".to_string(),
                value: EnumValue {
                    name: target_state_name.to_string(),
                },
            });
        }

        if custom_fields.is_empty() {
            eprintln!("Nothing to do in YouTrack - no version field, no target state.");
            return Ok(());
        }

        let body = PatchIssueBody {
            customFields: custom_fields,
        };

        let resp: Value = self.post_json(
            format!("issues/{issue_id}"),
            &body,
            &[("fields", "id,customFields(name,value(name))")],
        )?;

        // Do something with the requested fields?

        // Example success:
        // {"customFields":[{"value":null,"name":"Type","$type":"SingleEnumIssueCustomField"},{"value":{"name":"Released","$type":"StateBundleElement"},"name":"State","$type":"StateIssueCustomField"},{"value":null,"name":"Assignee","$type":"SingleUserIssueCustomField"},{"value":null,"name":"Priority","$type":"SingleEnumIssueCustomField"},{"value":{"name":"Internal tooling","$type":"EnumBundleElement"},"name":"Category","$type":"SingleEnumIssueCustomField"},{"value":[],"name":"Customer","$type":"MultiEnumIssueCustomField"},{"value":null,"name":"Customer Funding","$type":"SingleEnumIssueCustomField"},{"value":null,"name":"Product Stream","$type":"SingleEnumIssueCustomField"},{"value":null,"name":"Estimation","$type":"PeriodIssueCustomField"},{"value":{"$type":"PeriodValue"},"name":"Spent time","$type":"PeriodIssueCustomField"},{"value":null,"name":"Due Date","$type":"DateIssueCustomField"},{"value":[],"name":"Affected version","$type":"MultiVersionIssueCustomField"},{"value":{"name":"TEST2","$type":"VersionBundleElement"},"name":"Available in version","$type":"SingleVersionIssueCustomField"},{"value":null,"name":"SlackAlertSent","$type":"SimpleIssueCustomField"},{"value":13.0,"name":"Dev costs","$type":"SimpleIssueCustomField"}],"id":"2-25820","$type":"Issue"}

        println!("YouTrack issue {issue_id} updated.");

        debug!("Response to request to edit issue: {resp:?}");
        Ok(())
    }
}

/// Params for YT to change version field
#[derive(Clone)]
pub struct SetVersion<'a> {
    /// Field name, e.g. Available in version
    pub field_name: &'a str,
    /// Version name, e.g. 1.0.0
    pub version: &'a str,
}

#[cfg(test)]
mod tests {
    use super::{SetVersion, YouTrackClient};
    use crate::config::{ENV_YOUTRACK_TOKEN, ENV_YOUTRACK_URL};
    use log::{LevelFilter, debug};

    // #[test] // Disabled
    #[allow(unused)]
    fn test_youtrack_communication() {
        simple_logging::log_to_stderr(LevelFilter::Debug);

        let url = dotenv::var(ENV_YOUTRACK_URL).expect("Missing youtrack URL from env");
        let token = dotenv::var(ENV_YOUTRACK_TOKEN).expect("Missing youtrack token from env");

        // this must match the config in the connected youtrack
        let issue_id = "SW-4739";
        let version_field_name = "Available in version";
        let target_state_name = "Released";
        let version_name = "TEST2";

        let set_version = SetVersion {
            field_name: version_field_name,
            version: version_name,
        };

        let client = YouTrackClient::new(url, &token).unwrap();

        let project_id = client.find_project_id(issue_id).unwrap();

        debug!("Found YouTrack project ID = {project_id}");

        let date = chrono::Utc::now();

        client
            .ensure_version_exists_in_project(&project_id, &set_version, Some(date))
            .unwrap();

        client
            .set_issue_version_and_state_by_name(
                issue_id,
                Some(&set_version),
                Some(target_state_name),
            )
            .unwrap();
    }
}
