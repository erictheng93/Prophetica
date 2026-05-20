#![allow(clippy::expect_used, clippy::unwrap_used)]

use kyc_service::sumsub_documents::{SumsubDocument, SumsubDocumentClient};

#[tokio::test]
async fn lists_documents_for_applicant() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/mock/sumsub/applicants/applicant_1/documents",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "applicant_id": "applicant_1",
            "documents": [{ "doc_id": "doc_1", "kind": "id_doc", "country": "TWN",
                "side": "front", "content_type": "image/png", "captured_at": "2026-05-10T00:00:00Z" }]
        })))
        .mount(&server)
        .await;

    let client = SumsubDocumentClient::new(server.uri(), "app-token".into(), "secret-key".into());
    let docs: Vec<SumsubDocument> = client.list_documents("applicant_1").await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].doc_id, "doc_1");
    assert_eq!(docs[0].content_type, "image/png");
}

#[tokio::test]
async fn fetches_image_streaming() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/mock/sumsub/applicants/applicant_1/documents/doc_1/image",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_bytes(b"\x89PNG\r\n".as_ref())
                .append_header("content-type", "image/png"),
        )
        .mount(&server)
        .await;

    let client = SumsubDocumentClient::new(server.uri(), "app-token".into(), "secret-key".into());
    let image = client
        .fetch_image("applicant_1", "doc_1")
        .await
        .expect("fetch_image should succeed");
    assert_eq!(image.content_type, "image/png");
}
