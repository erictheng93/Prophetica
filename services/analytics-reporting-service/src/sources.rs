use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    error::ReportingError,
    types::{ReportParams, ReportTemplate, SourceProvenance, SourceSnapshot},
};

#[derive(Debug, Clone)]
pub struct SourceClients {
    http: reqwest::Client,
    kyc_url: Option<Url>,
    ledger_url: Option<Url>,
    risk_url: Option<Url>,
    user_url: Option<Url>,
}

impl SourceClients {
    #[must_use]
    pub fn fixtures() -> Self {
        Self {
            http: reqwest::Client::new(),
            kyc_url: None,
            ledger_url: None,
            risk_url: None,
            user_url: None,
        }
    }

    /// Creates source clients from service URL environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error when any required URL environment variable is missing or
    /// cannot be parsed as a URL.
    pub fn from_env() -> Result<Self, ReportingError> {
        Ok(Self {
            http: reqwest::Client::new(),
            kyc_url: Some(required_url_with_alias(
                "KYC_SERVICE_BASE_URL",
                "KYC_SERVICE_URL",
            )?),
            ledger_url: Some(required_url("LEDGER_SERVICE_URL")?),
            risk_url: Some(required_url("RISK_SERVICE_URL")?),
            user_url: Some(required_url("USER_SERVICE_URL")?),
        })
    }

    /// Resolves source rows and provenance for a report template.
    ///
    /// # Errors
    ///
    /// Returns an error when a configured upstream source URL is invalid, an
    /// upstream request fails, or the upstream response cannot be decoded.
    pub async fn resolve_rows(
        &self,
        template: &ReportTemplate,
        params: &ReportParams,
        request_id: &str,
    ) -> Result<(Vec<Value>, SourceSnapshot), ReportingError> {
        match template.code.as_str() {
            "ops-kpi-daily" => self.resolve_ops(template, params, request_id).await,
            "risk-alert-summary" => self.resolve_risk(template, params, request_id).await,
            "finance-reconciliation" => self.resolve_finance(template, params, request_id).await,
            _ => Ok((
                template.rows.clone(),
                fixture_snapshot(&template.code, request_id),
            )),
        }
    }

    async fn resolve_ops(
        &self,
        template: &ReportTemplate,
        params: &ReportParams,
        request_id: &str,
    ) -> Result<(Vec<Value>, SourceSnapshot), ReportingError> {
        let Some(kyc_url) = &self.kyc_url else {
            return Ok((
                template.rows.clone(),
                fixture_snapshot("kyc-service", request_id),
            ));
        };

        let kyc = self
            .get_source(
                kyc_url,
                "/v1/internal/reports/kyc-pass-rate",
                &[
                    ("from", params.start_at.as_str()),
                    ("to", params.end_at.as_str()),
                ],
                request_id,
            )
            .await?;
        let mut provenances = vec![kyc.source_snapshot.clone()];

        if let Some(ledger_url) = &self.ledger_url {
            let ledger = self
                .get_source(
                    ledger_url,
                    "/v1/internal/reports/reconciliation-snapshot",
                    &[("cutoff", params.end_at.as_str())],
                    request_id,
                )
                .await?;
            provenances.push(ledger.source_snapshot);
        }

        Ok((kyc.rows, SourceSnapshot::from_sources(provenances)))
    }

    async fn resolve_risk(
        &self,
        template: &ReportTemplate,
        params: &ReportParams,
        request_id: &str,
    ) -> Result<(Vec<Value>, SourceSnapshot), ReportingError> {
        let Some(risk_url) = &self.risk_url else {
            return Ok((
                template.rows.clone(),
                fixture_snapshot("risk-antifraud-service", request_id),
            ));
        };

        let risk = self
            .get_source(
                risk_url,
                "/v1/internal/reports/aml-alerts",
                &[
                    ("from", params.start_at.as_str()),
                    ("to", params.end_at.as_str()),
                ],
                request_id,
            )
            .await?;
        let rows = if let Some(user_url) = &self.user_url {
            self.attach_masked_contacts(user_url, risk.rows, request_id)
                .await?
        } else {
            risk.rows
        };

        Ok((
            rows,
            SourceSnapshot::from_sources(vec![risk.source_snapshot]),
        ))
    }

    async fn resolve_finance(
        &self,
        template: &ReportTemplate,
        params: &ReportParams,
        request_id: &str,
    ) -> Result<(Vec<Value>, SourceSnapshot), ReportingError> {
        let Some(ledger_url) = &self.ledger_url else {
            return Ok((
                template.rows.clone(),
                fixture_snapshot("wallet-ledger-service", request_id),
            ));
        };

        let ledger = self
            .get_source(
                ledger_url,
                "/v1/internal/reports/reconciliation-snapshot",
                &[("cutoff", params.end_at.as_str())],
                request_id,
            )
            .await?;
        Ok((
            ledger.rows,
            SourceSnapshot::from_sources(vec![ledger.source_snapshot]),
        ))
    }

    async fn get_source(
        &self,
        base_url: &Url,
        path: &str,
        query: &[(&str, &str)],
        request_id: &str,
    ) -> Result<SourceResponse, ReportingError> {
        let url = base_url
            .join(path)
            .map_err(|error| ReportingError::Upstream(error.to_string()))?;
        self.http
            .get(url)
            .query(query)
            .header("x-request-id", request_id)
            .send()
            .await
            .map_err(|error| ReportingError::Upstream(error.to_string()))?
            .error_for_status()
            .map_err(|error| ReportingError::Upstream(error.to_string()))?
            .json::<SourceResponse>()
            .await
            .map_err(|error| ReportingError::Upstream(error.to_string()))
    }

    async fn attach_masked_contacts(
        &self,
        user_url: &Url,
        rows: Vec<Value>,
        request_id: &str,
    ) -> Result<Vec<Value>, ReportingError> {
        let ids = rows
            .iter()
            .filter_map(|row| row.get("member_id").and_then(Value::as_str))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Ok(rows);
        }

        let url = user_url
            .join("/v1/internal/users/contacts:lookup-masked")
            .map_err(|error| ReportingError::Upstream(error.to_string()))?;
        let response = self
            .http
            .post(url)
            .header("x-request-id", request_id)
            .json(&serde_json::json!({ "ids": ids }))
            .send()
            .await
            .map_err(|error| ReportingError::Upstream(error.to_string()))?
            .error_for_status()
            .map_err(|error| ReportingError::Upstream(error.to_string()))?
            .json::<LookupResponse>()
            .await
            .map_err(|error| ReportingError::Upstream(error.to_string()))?;

        let contacts = response
            .contacts
            .into_iter()
            .map(|contact| (contact.id, contact.email_masked))
            .collect::<std::collections::HashMap<_, _>>();

        Ok(rows
            .into_iter()
            .map(|mut row| {
                if let Some(member_id) = row.get("member_id").and_then(Value::as_str)
                    && let Some(email) = contacts.get(member_id)
                    && let Some(object) = row.as_object_mut()
                {
                    object.insert("member_email".to_owned(), Value::String(email.clone()));
                }
                row
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct SourceResponse {
    rows: Vec<Value>,
    source_snapshot: SourceProvenance,
}

#[derive(Debug, Deserialize)]
struct LookupResponse {
    contacts: Vec<MaskedContact>,
}

#[derive(Debug, Deserialize)]
struct MaskedContact {
    id: String,
    email_masked: String,
}

fn required_url(name: &str) -> Result<Url, ReportingError> {
    let value =
        std::env::var(name).map_err(|_| ReportingError::Upstream(format!("{name} is required")))?;
    Url::parse(&value).map_err(|error| ReportingError::Upstream(error.to_string()))
}

fn required_url_with_alias(primary: &str, alias: &str) -> Result<Url, ReportingError> {
    let value = std::env::var(primary)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var(alias)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| ReportingError::Upstream(format!("{primary} or {alias} is required")))?;
    Url::parse(value.trim_end_matches('/'))
        .map_err(|error| ReportingError::Upstream(error.to_string()))
}

fn fixture_snapshot(service: &str, request_id: &str) -> SourceSnapshot {
    SourceSnapshot::single(
        service,
        "fixture.v2026.05.10",
        "2026-05-10T08:00:00+08:00",
        request_id,
    )
}
