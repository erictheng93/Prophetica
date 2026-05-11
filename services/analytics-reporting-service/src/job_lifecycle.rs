use sdk::masking::mask_email;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{
    error::ReportingError,
    types::{
        CreateReportJobRequest, ReportField, ReportParams, ReportRole, ReportTemplate,
        SourceSnapshot,
    },
};

const PLATFORM_REPORT_TIMEZONE: &str = "Asia/Taipei";
const UTC_REPORT_TIMEZONE: &str = "UTC";

/// Validates whether a report request is permitted for a template.
///
/// # Errors
///
/// Returns an error when the caller role is not allowed, sensitive-field reveal
/// lacks required authorization, or requested sort/column fields are invalid.
pub fn validate(
    template: &ReportTemplate,
    request: &CreateReportJobRequest,
) -> Result<(), ReportingError> {
    let role = request.role.unwrap_or(ReportRole::Manager);
    if !template.viewer_roles.contains(&role) || !role.allows_admin_reporting() {
        return Err(ReportingError::Forbidden);
    }
    match request.params.timezone.as_str() {
        PLATFORM_REPORT_TIMEZONE => {}
        UTC_REPORT_TIMEZONE if role == ReportRole::Admin => {}
        UTC_REPORT_TIMEZONE => return Err(ReportingError::Forbidden),
        _ => {
            return Err(ReportingError::InvalidRequest(
                "unsupported report timezone".to_owned(),
            ));
        }
    }
    if request.reveal_sensitive_fields.unwrap_or(false)
        && (role != ReportRole::Admin
            || request
                .second_authorization_id
                .as_ref()
                .is_none_or(|id| id.trim().is_empty()))
    {
        return Err(ReportingError::Forbidden);
    }

    let field_keys = template
        .fields
        .iter()
        .map(|field| field.key.as_str())
        .collect::<Vec<_>>();
    if !field_keys.contains(&request.params.sort_by.as_str()) {
        return Err(ReportingError::InvalidRequest(format!(
            "unknown sort field {}",
            request.params.sort_by
        )));
    }
    if request
        .params
        .columns
        .iter()
        .any(|column| !field_keys.contains(&column.as_str()))
    {
        return Err(ReportingError::InvalidRequest(
            "unknown report column".to_owned(),
        ));
    }

    Ok(())
}

#[must_use]
pub fn sort_rows(rows: &[Value], sort_by: &str, direction: &str) -> Vec<Value> {
    let mut sorted = rows.to_vec();
    sorted.sort_by(|left, right| {
        let left_value = left
            .get(sort_by)
            .map_or_else(String::new, value_to_sort_key);
        let right_value = right
            .get(sort_by)
            .map_or_else(String::new, value_to_sort_key);
        if direction == "desc" {
            right_value.cmp(&left_value)
        } else {
            left_value.cmp(&right_value)
        }
    });
    sorted
}

#[must_use]
pub fn project_rows(
    rows: &[Value],
    fields: &[ReportField],
    columns: &[String],
    reveal_sensitive_fields: bool,
) -> Vec<Value> {
    let sensitive_fields = fields
        .iter()
        .filter(|field| field.sensitive)
        .map(|field| field.key.as_str())
        .collect::<Vec<_>>();

    rows.iter()
        .map(|row| {
            let mut projected = Map::new();
            for column in columns {
                let value = row.get(column).cloned().unwrap_or(Value::Null);
                if sensitive_fields.contains(&column.as_str()) && !reveal_sensitive_fields {
                    projected.insert(column.clone(), mask_sensitive_value(&value));
                } else {
                    projected.insert(column.clone(), value);
                }
            }
            Value::Object(projected)
        })
        .collect()
}

#[must_use]
pub fn row_hash(
    template_code: &str,
    params: &ReportParams,
    source_snapshot: &SourceSnapshot,
    rows: &[Value],
) -> String {
    let canonical = canonical_json(&json!({
        "template_code": template_code,
        "params": params,
        "source_snapshot": source_snapshot,
        "rows": rows
    }));
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn mask_sensitive_value(value: &Value) -> Value {
    value
        .as_str()
        .map_or_else(|| json!("****"), |text| json!(mask_email(text)))
}

fn value_to_sort_key(value: &Value) -> String {
    match value {
        Value::Number(number) => format!("{number:0>20}"),
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null | Value::Array(_) | Value::Object(_) => String::new(),
    }
}

fn canonical_json(value: &Value) -> String {
    sort_json(value).to_string()
}

fn sort_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sort_json).collect()),
        Value::Object(map) => {
            let mut sorted = Map::new();
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(nested) = map.get(key) {
                    sorted.insert(key.clone(), sort_json(nested));
                }
            }
            Value::Object(sorted)
        }
        primitive => primitive.clone(),
    }
}
