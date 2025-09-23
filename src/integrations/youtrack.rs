//! Youtrack integration (mark issues as Released when packing to changelog)

use crate::config::VersionName;
use anyhow::bail;
use chrono::{DateTime, Utc};
use json_dotpath::DotPaths;
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;

type ProjectId = String;
type BundleID = String;
type FieldID = String;

pub struct YouTrackClient {
    client: reqwest::blocking::Client,
    pub url: String,
}

#[derive(Deserialize)]
struct YoutrackErrorResponse {
    error: String,
    error_description: String,
}

impl YouTrackClient {
    pub fn new(url: impl ToString, token: &str) -> anyhow::Result<Self> {
        let token_bearer = format!("Bearer {token}");

        let mut hm = HeaderMap::new();
        hm.insert("Authorization", HeaderValue::from_str(&token_bearer)?);
        hm.insert("Content-Type", HeaderValue::from_str("application/json")?);
        hm.insert("Accept", HeaderValue::from_str("application/json")?);

        Ok(YouTrackClient {
            url: url.to_string(),
            client: reqwest::blocking::Client::builder()
                .default_headers(hm)
                .build()?,
        })
    }

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

        let response_text = response.text()?;

        debug!("Resp = {}", response_text);

        if let Ok(e) = serde_json::from_str::<YoutrackErrorResponse>(&response_text) {
            bail!("Error from YouTrack: {} - {}", e.error, e.error_description);
        }

        Ok(serde_json::from_str(&response_text)?)
    }

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

        let response_text = response.text()?;

        debug!("Resp = {}", response_text);

        if let Ok(e) = serde_json::from_str::<YoutrackErrorResponse>(&response_text) {
            bail!("Error from YouTrack: {} - {}", e.error, e.error_description);
        }

        Ok(serde_json::from_str(&response_text)?)
    }

    pub fn find_project_id(&self, issue: &str) -> anyhow::Result<ProjectId> {
        #[derive(Deserialize)]
        struct Issue {
            project: Project,
        }

        #[derive(Deserialize)]
        struct Project {
            id: ProjectId,
        }

        let issue: Issue =
            self.get_json(format!("issues/{issue}"), &[("fields", "project(id)")])?;

        // example:
        // {"project":{"id":"0-172","$type":"Project"},"$type":"Issue"}

        Ok(issue.project.id)
    }

    /// Try to find a version by name in a YouTrack project.
    /// If it is not found but we find the field where to add it, the version will be created.
    ///
    /// project_id - obtained by `find_project_id()`
    /// field_name - name of the version field, e.g. "Available in version"
    /// version_to_create - name of the version to find or create
    /// release_date - if creating, a date YYYY-MM-DD can be passed here. It will be stored into the
    ///   newly created version & this marked as released.
    pub fn ensure_version_exists_in_project(
        &self,
        project_id: &str,
        field_name: &str,
        version_to_create: &str,
        release_date: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
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

        let mut field_bundle = None;
        for entry in fields {
            if &entry.field.name == field_name
                && let Some(bundle) = entry.bundle
            {
                field_bundle = Some((entry.field.id, bundle.id));
                break;
            }
        }

        let Some((field_id, bundle_id)) = field_bundle else {
            bail!("YouTrack version field {field_name} not found in the project {project_id}");
        };

        println!("Found YouTrack version field, checking defined versions");

        #[derive(Deserialize)]
        struct YTVersion {
            name: VersionName,
            id: String,
        }

        let versions: Vec<YTVersion> = self.get_json(
            format!("admin/customFieldSettings/bundles/version/{bundle_id}/values"),
            &[("fields", "id,name"), ("top", "500")],
        )?;

        // Find the version we want
        for version in versions {
            if &version.name == version_to_create {
                eprintln!("Version {version_to_create} already exists in YouTrack");
                return Ok(());
            }
        }

        println!("Creating version in YouTrack: {version_to_create}");

        /*
            $body = ['name' => $name];
            if ($released !== null) {
                $body['released'] = $released;
            }
            if ($releaseDate !== null) {
                $body['releaseDate'] = $releaseDate;
            }
            if ($archived !== null) {
                $body['archived'] = $archived;
            }

            return $this->postJson(
                "admin/customFieldSettings/bundles/version/$bundleId/values",
                $body,
                ['fields' => 'id,name,released,releaseDate,archived'],
            );
        */

        #[derive(Serialize)]
        struct CreateVersionBody {
            name: String,
            archived: bool,
            released: bool,
            #[allow(non_snake_case)]
            releaseDate: Option<i64>,
            // archived
        }

        let body = CreateVersionBody {
            name: version_to_create.to_string(),
            archived: false,
            released: release_date.is_some(),
            releaseDate: release_date.map(|d| d.timestamp()),
        };

        #[derive(Deserialize, Debug)]
        struct CreateVersionResponse {
            #[allow(non_snake_case)]
            releaseDate: Option<i64>,
            released: bool,
            archived: bool,
            name: String,
            id: String,
        }

        let resp: CreateVersionResponse = self.post_json(
            format!("admin/customFieldSettings/bundles/version/{bundle_id}/values"),
            &body,
            &[("fields", "id,name,released,releaseDate,archived")],
        )?;

        // Example response:
        // {"releaseDate":null,"released":false,"archived":false,"name":"TEST1","id":"232-356","$type":"VersionBundleElement"}
        // {"releaseDate":1758619201,"released":true,"archived":false,"name":"TEST2","id":"232-358","$type":"VersionBundleElement"}

        debug!("Created version entry = {:#?}", resp);
        println!("Version {version_to_create} created in YouTrack.");

        Ok(())
    }

    pub fn set_issue_version_and_state(
        &self,
        issue_id: &str,
        version_field_name: &str,
        version_name: &str,
        target_state_name: &str,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct PatchIssueBody {
            #[allow(non_snake_case)]
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

        let body = PatchIssueBody {
            customFields: vec![
                CustomFieldValue {
                    name: version_field_name.to_string(),
                    field_type: "SingleVersionIssueCustomField".to_string(),
                    value: EnumValue {
                        name: version_name.to_string(),
                    },
                },
                CustomFieldValue {
                    name: "State".to_string(),
                    field_type: "StateIssueCustomField".to_string(),
                    value: EnumValue {
                        name: target_state_name.to_string(),
                    },
                },
            ],
        };

        let resp: Value = self.post_json(
            format!("issues/{issue_id}"),
            &body,
            &[("fields", "id,customFields(name,value(name))")],
        )?;

        // TODO? Do something with the fields

        // Example success:
        // {"customFields":[{"value":null,"name":"Type","$type":"SingleEnumIssueCustomField"},{"value":{"name":"Released","$type":"StateBundleElement"},"name":"State","$type":"StateIssueCustomField"},{"value":null,"name":"Assignee","$type":"SingleUserIssueCustomField"},{"value":null,"name":"Priority","$type":"SingleEnumIssueCustomField"},{"value":{"name":"Internal tooling","$type":"EnumBundleElement"},"name":"Category","$type":"SingleEnumIssueCustomField"},{"value":[],"name":"Customer","$type":"MultiEnumIssueCustomField"},{"value":null,"name":"Customer Funding","$type":"SingleEnumIssueCustomField"},{"value":null,"name":"Product Stream","$type":"SingleEnumIssueCustomField"},{"value":null,"name":"Estimation","$type":"PeriodIssueCustomField"},{"value":{"$type":"PeriodValue"},"name":"Spent time","$type":"PeriodIssueCustomField"},{"value":null,"name":"Due Date","$type":"DateIssueCustomField"},{"value":[],"name":"Affected version","$type":"MultiVersionIssueCustomField"},{"value":{"name":"TEST2","$type":"VersionBundleElement"},"name":"Available in version","$type":"SingleVersionIssueCustomField"},{"value":null,"name":"SlackAlertSent","$type":"SimpleIssueCustomField"},{"value":13.0,"name":"Dev costs","$type":"SimpleIssueCustomField"}],"id":"2-25820","$type":"Issue"}

        println!("YouTrack issue {issue_id} updated.");

        debug!("Response to request to edit issue: {resp:?}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::YouTrackClient;
    use crate::config::{ENV_YOUTRACK_TOKEN, ENV_YOUTRACK_URL};
    use log::{LevelFilter, debug};
    use serde_json::Value;

    // #[test] // Disabled
    fn test_youtrack_communication() {
        simple_logging::log_to_stderr(LevelFilter::Debug);

        let url = dotenv::var(ENV_YOUTRACK_URL).expect("Missing youtrack URL from env");
        let token = dotenv::var(ENV_YOUTRACK_TOKEN).expect("Missing youtrack token from env");

        // this must match the config in the connected youtrack
        let issue_id = "SW-4739";
        let version_field_name = "Available in version";
        let target_state_name = "Released";
        let version_name = "TEST2";

        let mut client = YouTrackClient::new(url, &token).unwrap();

        let project_id = client.find_project_id(issue_id).unwrap();

        debug!("Found YouTrack project ID = {project_id}");

        let date = chrono::Utc::now();

        client
            .ensure_version_exists_in_project(
                &project_id,
                version_field_name,
                version_name,
                Some(date),
            )
            .unwrap();

        client
            .set_issue_version_and_state(
                issue_id,
                version_field_name,
                version_name,
                target_state_name,
            )
            .unwrap();
    }
}
