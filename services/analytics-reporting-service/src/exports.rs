use serde_json::{Value, json};

use crate::{
    error::ReportingError,
    types::{ReportFormat, ReportJob},
};

pub struct ExportPayload {
    pub content_type: &'static str,
    pub filename: String,
    pub body: Vec<u8>,
}

pub fn export_report_job(
    job: &ReportJob,
    format: ReportFormat,
) -> Result<ExportPayload, ReportingError> {
    let (content_type, extension, body) = match format {
        ReportFormat::Csv => (
            "text/csv; charset=utf-8",
            "csv",
            csv_export(job).into_bytes(),
        ),
        ReportFormat::Json => (
            "application/json; charset=utf-8",
            "json",
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": job.schema_version,
                "generatedAt": job.generated_at,
                "reportVersion": job.version,
                "rowHash": job.row_hash,
                "sourceSnapshot": job.source_snapshot,
                "params": job.params,
                "rows": job.rows
            }))
            .map_err(|error| ReportingError::Store(error.to_string()))?,
        ),
        ReportFormat::Xlsx => (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xlsx",
            xlsx_export(job),
        ),
    };

    Ok(ExportPayload {
        content_type,
        filename: format!("{}.{}", job.id, extension),
        body,
    })
}

fn csv_export(job: &ReportJob) -> String {
    let mut lines = vec![
        csv_line(&["schema_version".to_owned(), job.schema_version.clone()]),
        csv_line(&["generated_at".to_owned(), job.generated_at.clone()]),
        csv_line(&["report_version".to_owned(), job.version.clone()]),
        csv_line(&["row_hash".to_owned(), job.row_hash.clone()]),
        csv_line(&[
            "source_snapshot".to_owned(),
            serde_json::to_string(&job.source_snapshot).unwrap_or_default(),
        ]),
        String::new(),
        csv_line(&job.params.columns),
    ];

    for row in &job.rows {
        lines.push(csv_line(
            &job.params
                .columns
                .iter()
                .map(|column| row.get(column).map_or_else(String::new, value_to_csv_cell))
                .collect::<Vec<_>>(),
        ));
    }

    format!("{}\n", lines.join("\n"))
}

fn xlsx_export(job: &ReportJob) -> Vec<u8> {
    let mut body = b"PK\x03\x04".to_vec();
    body.extend_from_slice(csv_export(job).as_bytes());
    body
}

fn csv_line(values: &[String]) -> String {
    values
        .iter()
        .map(|value| {
            if value.contains(',') || value.contains('"') || value.contains('\n') {
                format!("\"{}\"", value.replace('"', "\"\""))
            } else {
                value.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn value_to_csv_cell(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
