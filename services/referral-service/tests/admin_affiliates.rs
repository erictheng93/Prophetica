#![allow(clippy::expect_used)]

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use referral_service::{
    AffiliateCreditProfile, AffiliateDetail, AffiliateSummary, CommissionPayoutLedgerDraftResponse,
    DueQualificationReferral, InMemoryAffiliateDashboardStore, QualificationEvent,
    QualificationInputProvider, ReferralDailyWorker, ReferralLedgerPoster, ReferralStoreError,
    SignupBonusLedgerDraftResponse, router,
};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn capture_referral_is_first_touch_and_idempotent() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    let created = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = response_json(created).await;
    assert_eq!(body["referrer_id"], "user_aff_100");
    assert_eq!(body["referred_id"], "user_new_1");
    assert_eq!(body["referral_code"], "AFF-100");
    assert_eq!(body["status"], "pending");
    assert_eq!(body["captured"], true);

    let replayed = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_001
            }),
        ))
        .await
        .expect("replay should be handled");
    assert_eq!(replayed.status(), StatusCode::OK);
    let body = response_json(replayed).await;
    assert_eq!(body["referred_at_day"], 20_000);
    assert_eq!(body["captured"], false);
}

#[tokio::test]
async fn capture_referral_rejects_self_referral() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    let response = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_aff_100",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("self-referral should be handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        "invalid_affiliate_request"
    );
}

#[tokio::test]
async fn qualification_evaluator_qualifies_matured_clean_referral() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    let captured = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    assert_eq!(captured.status(), StatusCode::CREATED);

    let evaluated = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/qualification-evaluate",
            &json!({
                "referred_id": "user_new_1",
                "observed_at_day": 20_030,
                "deposit_minor_units": 50_000,
                "kyc_tier": 1,
                "completed_trade_count": 5,
                "anti_abuse": {
                    "same_ip": false,
                    "same_device_fingerprint": false,
                    "same_kyc_identity": false,
                    "referred_self_trade": false,
                    "chargeback_or_void": false,
                    "referrer_account_abnormal": false,
                    "referred_permanently_banned": false
                },
                "kinship_review_passed": false
            }),
        ))
        .await
        .expect("evaluation should be handled");

    assert_eq!(evaluated.status(), StatusCode::OK);
    let body = response_json(evaluated).await;
    assert_eq!(body["status"], "qualified");
    assert_eq!(body["qualified_at_day"], 20_030);
    assert_eq!(body["signup_bonus_minor_units"], 10_000);
    assert_eq!(body["referred_signup_bonus_minor_units"], 5_000);
}

#[tokio::test]
async fn signup_bonus_finalization_records_ledger_txn_idempotently() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    let captured = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    assert_eq!(captured.status(), StatusCode::CREATED);
    let evaluated = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/qualification-evaluate",
            &qualification_event_json(),
        ))
        .await
        .expect("evaluation should be handled");
    assert_eq!(response_json(evaluated).await["status"], "qualified");

    let draft = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/internal/referrals/signup-bonus-ledger-draft/user_new_1")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("draft should be handled");
    assert_eq!(draft.status(), StatusCode::OK);
    let body = response_json(draft).await;
    assert_eq!(
        body["idempotency_key"],
        "referral-signup-bonus:user_aff_100:user_new_1"
    );
    assert_eq!(body["txn_type"], "referral_signup_bonus");
    assert_eq!(body["entries"].as_array().expect("entries array").len(), 4);
    assert_eq!(body["entries"][0]["direction"], "debit");
    assert_eq!(body["entries"][1]["direction"], "credit");

    let finalized = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/signup-bonus-finalize",
            &json!({
                "referred_id": "user_new_1",
                "signup_bonus_txn_id": 9001,
                "posted_at_day": 20_031
            }),
        ))
        .await
        .expect("finalization should be handled");
    assert_eq!(finalized.status(), StatusCode::OK);
    let body = response_json(finalized).await;
    assert_eq!(body["status"], "qualified");
    assert_eq!(body["signup_bonus_txn_id"], 9001);
    assert_eq!(body["replayed"], false);

    let replayed = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/signup-bonus-finalize",
            &json!({
                "referred_id": "user_new_1",
                "signup_bonus_txn_id": 9001,
                "posted_at_day": 20_031
            }),
        ))
        .await
        .expect("replay should be handled");
    assert_eq!(replayed.status(), StatusCode::OK);
    assert_eq!(response_json(replayed).await["replayed"], true);

    let conflicting = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/signup-bonus-finalize",
            &json!({
                "referred_id": "user_new_1",
                "signup_bonus_txn_id": 9002,
                "posted_at_day": 20_031
            }),
        ))
        .await
        .expect("conflict should be handled");
    assert_eq!(conflicting.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(conflicting).await["message"],
        "invalid affiliate request: signup bonus ledger transaction already finalized"
    );
}

#[tokio::test]
async fn qualification_due_lists_only_matured_pending_referrals() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    for (referred_user_id, referred_at_day) in [("user_due", 20_000), ("user_not_due", 20_010)] {
        let captured = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/internal/referrals/capture",
                &json!({
                    "referral_code": "AFF-100",
                    "referred_user_id": referred_user_id,
                    "referred_at_day": referred_at_day
                }),
            ))
            .await
            .expect("capture should be handled");
        assert_eq!(captured.status(), StatusCode::CREATED);
    }

    let due = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/internal/referrals/qualification-due?observed_at_day=20030&limit=100")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("due query should be handled");

    assert_eq!(due.status(), StatusCode::OK);
    let body = response_json(due).await;
    let due_referrals = body.as_array().expect("due response is an array");
    assert_eq!(due_referrals.len(), 1);
    assert_eq!(due_referrals[0]["referred_id"], "user_due");
    assert_eq!(due_referrals[0]["status"], "pending");
}

#[tokio::test]
async fn daily_worker_evaluates_due_referrals_and_finalizes_signup_bonus() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store.clone());

    let captured = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    assert_eq!(captured.status(), StatusCode::CREATED);

    let worker = ReferralDailyWorker::new(store, CleanQualificationInputs, FixedLedgerPoster);
    let report = worker
        .run_qualification_batch(20_030, 100)
        .await
        .expect("worker should process due referrals");

    assert_eq!(report.scanned_count, 1);
    assert_eq!(report.qualified_count, 1);
    assert_eq!(report.signup_bonus_posted_count, 1);
}

#[tokio::test]
async fn qualification_evaluator_sends_signup_cap_overflow_to_edd_review() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    for index in 0..20 {
        let referred_user_id = format!("user_new_{index}");
        app.clone()
            .oneshot(json_request(
                "POST",
                "/internal/referrals/capture",
                &json!({
                    "referral_code": "AFF-100",
                    "referred_user_id": referred_user_id,
                    "referred_at_day": 20_000
                }),
            ))
            .await
            .expect("capture should be handled");
        let evaluated = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/internal/referrals/qualification-evaluate",
                &json!({
                    "referred_id": format!("user_new_{index}"),
                    "observed_at_day": 20_030,
                    "deposit_minor_units": 50_000,
                    "kyc_tier": 1,
                    "completed_trade_count": 5,
                    "anti_abuse": clean_anti_abuse_json(),
                    "kinship_review_passed": false
                }),
            ))
            .await
            .expect("evaluation should be handled");
        assert_eq!(response_json(evaluated).await["status"], "qualified");
    }

    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_over_cap",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    let capped = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/qualification-evaluate",
            &json!({
                "referred_id": "user_new_over_cap",
                "observed_at_day": 20_030,
                "deposit_minor_units": 50_000,
                "kyc_tier": 1,
                "completed_trade_count": 5,
                "anti_abuse": clean_anti_abuse_json(),
                "kinship_review_passed": false
            }),
        ))
        .await
        .expect("capped evaluation should be handled");

    let body = response_json(capped).await;
    assert_eq!(body["status"], "pending_eddreview");
    assert_eq!(body["signup_bonus_minor_units"], 0);
}

#[tokio::test]
async fn commission_accrual_requires_qualified_referral_and_is_idempotent_by_trade() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");

    let unqualified = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals",
            &commission_event_json("trade_1"),
        ))
        .await
        .expect("unqualified accrual should be handled");
    assert_eq!(unqualified.status(), StatusCode::BAD_REQUEST);

    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/qualification-evaluate",
            &qualification_event_json(),
        ))
        .await
        .expect("evaluation should be handled");

    let accrued = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals",
            &commission_event_json("trade_1"),
        ))
        .await
        .expect("accrual should be handled");
    assert_eq!(accrued.status(), StatusCode::OK);
    let body = response_json(accrued).await;
    assert_eq!(body["trade_id"], "trade_1");
    assert_eq!(body["fee_amount_minor_units"], 3_000);
    assert_eq!(body["commission_amount_minor_units"], 300);

    let replayed = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals",
            &commission_event_json("trade_1"),
        ))
        .await
        .expect("replay should be handled");
    assert_eq!(replayed.status(), StatusCode::OK);
    assert_eq!(
        response_json(replayed).await["commission_amount_minor_units"],
        300
    );
}

#[tokio::test]
async fn daily_worker_posts_commission_payout_and_marks_accruals_paid() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store.clone());

    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/qualification-evaluate",
            &qualification_event_json(),
        ))
        .await
        .expect("evaluation should be handled");
    for trade_id in ["trade_1", "trade_2"] {
        app.clone()
            .oneshot(json_request(
                "POST",
                "/internal/referrals/commission-accruals",
                &commission_event_json(trade_id),
            ))
            .await
            .expect("accrual should be handled");
    }

    let worker = ReferralDailyWorker::new(store, CleanQualificationInputs, FixedLedgerPoster);
    let reports = worker
        .run_due_commission_payouts(20_032, 100)
        .await
        .expect("worker should post due payouts");

    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.referrer_id, "user_aff_100");
    assert_eq!(report.paid_out_txn_id, 9_902);
    assert_eq!(report.accrual_count, 2);
    assert_eq!(report.total_commission_minor_units, 600);
}

#[tokio::test]
async fn commission_payout_marks_unpaid_accruals_and_is_idempotent_by_ledger_txn() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/qualification-evaluate",
            &qualification_event_json(),
        ))
        .await
        .expect("evaluation should be handled");
    for trade_id in ["trade_1", "trade_2"] {
        app.clone()
            .oneshot(json_request(
                "POST",
                "/internal/referrals/commission-accruals",
                &commission_event_json(trade_id),
            ))
            .await
            .expect("accrual should be handled");
    }

    let due = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/internal/referrals/commission-accruals-payout-due?limit=100")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("due payout query should be handled");
    assert_eq!(due.status(), StatusCode::OK);
    let body = response_json(due).await;
    let due_referrers = body.as_array().expect("due payout response is an array");
    assert_eq!(due_referrers.len(), 1);
    assert_eq!(due_referrers[0]["referrer_id"], "user_aff_100");
    assert_eq!(due_referrers[0]["accrual_count"], 2);
    assert_eq!(due_referrers[0]["total_commission_minor_units"], 600);

    let draft = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/internal/referrals/commission-accruals-payout-ledger-draft/user_aff_100?paid_out_at_day=20032",
                )
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("payout draft should be handled");
    assert_eq!(draft.status(), StatusCode::OK);
    let body = response_json(draft).await;
    assert_eq!(
        body["idempotency_key"],
        "referral-commission-payout:user_aff_100:20032"
    );
    assert_eq!(body["txn_type"], "referral_commission_payout");
    assert_eq!(body["accrual_count"], 2);
    assert_eq!(body["total_commission_minor_units"], 600);
    assert_eq!(body["entries"].as_array().expect("entries array").len(), 2);

    let payout = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals-payout",
            &json!({
                "referrer_id": "user_aff_100",
                "paid_out_txn_id": 77,
                "paid_out_at_day": 20_032
            }),
        ))
        .await
        .expect("payout should be handled");
    assert_eq!(payout.status(), StatusCode::OK);
    let body = response_json(payout).await;
    assert_eq!(body["accrual_count"], 2);
    assert_eq!(body["total_commission_minor_units"], 600);

    let replayed = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals-payout",
            &json!({
                "referrer_id": "user_aff_100",
                "paid_out_txn_id": 77,
                "paid_out_at_day": 20_032
            }),
        ))
        .await
        .expect("payout replay should be handled");
    assert_eq!(replayed.status(), StatusCode::OK);
    assert_eq!(
        response_json(replayed).await["total_commission_minor_units"],
        600
    );
}

#[tokio::test]
async fn commission_accrual_rejects_referrer_monthly_cap_overflow() {
    let store = InMemoryAffiliateDashboardStore::new(
        [("user_aff_100".to_owned(), "AFF-100".to_owned())].into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/capture",
            &json!({
                "referral_code": "AFF-100",
                "referred_user_id": "user_new_1",
                "referred_at_day": 20_000
            }),
        ))
        .await
        .expect("capture should be handled");
    app.clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/qualification-evaluate",
            &qualification_event_json(),
        ))
        .await
        .expect("evaluation should be handled");

    let almost_capped = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals",
            &json!({
                "referred_id": "user_new_1",
                "trade_id": "trade_near_cap",
                "traded_at_day": 20_031,
                "maker_fee_minor_units": 49_999_000,
                "taker_fee_minor_units": 0,
                "anti_abuse": clean_anti_abuse_json(),
                "referrer_group_id": null
            }),
        ))
        .await
        .expect("near-cap accrual should be handled");
    assert_eq!(almost_capped.status(), StatusCode::OK);

    let overflow = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals",
            &commission_event_json("trade_over_cap"),
        ))
        .await
        .expect("overflow accrual should be handled");
    assert_eq!(overflow.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(overflow).await["message"],
        "invalid affiliate request: monthly trade commission cap requires EDD review"
    );
}

#[tokio::test]
async fn commission_accrual_rejects_group_monthly_cap_overflow() {
    let store = InMemoryAffiliateDashboardStore::new(
        [
            ("user_aff_100".to_owned(), "AFF-100".to_owned()),
            ("user_aff_200".to_owned(), "AFF-200".to_owned()),
        ]
        .into(),
        Default::default(),
        Default::default(),
    );
    let app = router(store);

    for (code, user_id) in [("AFF-100", "user_new_1"), ("AFF-200", "user_new_2")] {
        app.clone()
            .oneshot(json_request(
                "POST",
                "/internal/referrals/capture",
                &json!({
                    "referral_code": code,
                    "referred_user_id": user_id,
                    "referred_at_day": 20_000
                }),
            ))
            .await
            .expect("capture should be handled");
        app.clone()
            .oneshot(json_request(
                "POST",
                "/internal/referrals/qualification-evaluate",
                &json!({
                    "referred_id": user_id,
                    "observed_at_day": 20_030,
                    "deposit_minor_units": 50_000,
                    "kyc_tier": 1,
                    "completed_trade_count": 5,
                    "anti_abuse": clean_anti_abuse_json(),
                    "kinship_review_passed": false
                }),
            ))
            .await
            .expect("evaluation should be handled");
    }

    let near_group_cap = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals",
            &json!({
                "referred_id": "user_new_1",
                "trade_id": "trade_group_near_cap",
                "traded_at_day": 20_031,
                "maker_fee_minor_units": 49_999_000,
                "taker_fee_minor_units": 0,
                "anti_abuse": clean_anti_abuse_json(),
                "referrer_group_id": "risk_group_1"
            }),
        ))
        .await
        .expect("near group cap accrual should be handled");
    assert_eq!(near_group_cap.status(), StatusCode::OK);

    let overflow = app
        .oneshot(json_request(
            "POST",
            "/internal/referrals/commission-accruals",
            &json!({
                "referred_id": "user_new_2",
                "trade_id": "trade_group_over_cap",
                "traded_at_day": 20_031,
                "maker_fee_minor_units": 1_000,
                "taker_fee_minor_units": 2_000,
                "anti_abuse": clean_anti_abuse_json(),
                "referrer_group_id": "risk_group_1"
            }),
        ))
        .await
        .expect("group overflow accrual should be handled");
    assert_eq!(overflow.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(overflow).await["message"],
        "invalid affiliate request: group monthly trade commission cap requires EDD review"
    );
}

#[tokio::test]
async fn admin_affiliate_crud_mutates_with_audit_context() {
    let store = InMemoryAffiliateDashboardStore::default().with_admin_affiliates(vec![sample()]);
    let app = router(store);

    let created = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/admin/affiliates",
            &json!({
                "id": "AFF-200",
                "userId": "user_aff_200",
                "displayName": "Orion Partners",
                "emailMasked": "o***@orion.partners",
                "status": "onboarding",
                "commissionRateBps": 600,
                "tier": 1,
                "creditRating": "restricted",
                "creditStatus": "pending_review",
                "reason": "signed B2B onboarding packet",
                "authorizationBasis": "SPEC-011 §4.7"
            }),
        ))
        .await
        .expect("create should be handled");
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = response_json(created).await;
    assert_eq!(body["affiliate"]["id"], "AFF-200");
    assert_eq!(body["affiliate"]["displayName"], "Orion Partners");
    assert_eq!(
        body["auditCorrelationId"],
        "audit:affiliate-create:AFF-200:req-1"
    );

    let updated = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            "/internal/admin/affiliates/AFF-200",
            &json!({
                "displayName": "Orion Growth Partners",
                "commissionRateBps": 700,
                "creditRating": "B",
                "creditStatus": "active",
                "maxActiveCreditAccounts": 4,
                "totalCreditLimitMinorUnits": 500_000,
                "perAccountLimitMinorUnits": 125_000,
                "reason": "KYB approved and contract amended",
                "authorizationBasis": "admin + compliance approval",
                "effectiveAt": "2026-05-15"
            }),
        ))
        .await
        .expect("update should be handled");
    assert_eq!(updated.status(), StatusCode::OK);
    let body = response_json(updated).await;
    assert_eq!(body["affiliate"]["displayName"], "Orion Growth Partners");
    assert_eq!(body["affiliate"]["commissionRateBps"], 700);
    assert_eq!(body["affiliate"]["creditProfile"]["rating"], "B");
    assert_eq!(body["affiliate"]["rateHistory"][0]["previousBps"], 600);

    let archived = app
        .clone()
        .oneshot(json_request(
            "DELETE",
            "/internal/admin/affiliates/AFF-200",
            &json!({
                "reason": "offboarding requested by legal",
                "authorizationBasis": "B2B termination notice"
            }),
        ))
        .await
        .expect("archive should be handled");
    assert_eq!(archived.status(), StatusCode::OK);
    let body = response_json(archived).await;
    assert_eq!(body["affiliate"]["status"], "archived");
    assert_eq!(body["affiliate"]["creditProfile"]["status"], "closed");

    let list = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/internal/admin/affiliates")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("list should be handled");
    let body = response_json(list).await;
    assert_eq!(body.as_array().expect("list is array").len(), 1);
    assert_eq!(body[0]["id"], "AFF-100");
}

#[tokio::test]
async fn manager_cannot_mutate_credit_controls() {
    let store = InMemoryAffiliateDashboardStore::default().with_admin_affiliates(vec![sample()]);
    let app = router(store);

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/internal/admin/affiliates/AFF-100")
                .header("x-actor-id", "mgr-1")
                .header("x-operator-role", "manager")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "creditRating": "A",
                        "reason": "manager proposal",
                        "authorizationBasis": "manager review"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"],
        "affiliate_forbidden"
    );
}

fn json_request(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-actor-id", "admin-1")
        .header("x-operator-role", "admin")
        .header("x-request-id", "req-1")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

async fn response_json(response: axum::response::Response) -> Value {
    let collected = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect");
    serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
}

fn qualification_event_json() -> Value {
    json!({
        "referred_id": "user_new_1",
        "observed_at_day": 20_030,
        "deposit_minor_units": 50_000,
        "kyc_tier": 1,
        "completed_trade_count": 5,
        "anti_abuse": clean_anti_abuse_json(),
        "kinship_review_passed": false
    })
}

fn commission_event_json(trade_id: &str) -> Value {
    json!({
        "referred_id": "user_new_1",
        "trade_id": trade_id,
        "traded_at_day": 20_031,
        "maker_fee_minor_units": 1_000,
        "taker_fee_minor_units": 2_000,
        "anti_abuse": clean_anti_abuse_json(),
        "referrer_group_id": null
    })
}

fn clean_anti_abuse_json() -> Value {
    json!({
        "same_ip": false,
        "same_device_fingerprint": false,
        "same_kyc_identity": false,
        "referred_self_trade": false,
        "chargeback_or_void": false,
        "referrer_account_abnormal": false,
        "referred_permanently_banned": false
    })
}

struct CleanQualificationInputs;

#[async_trait]
impl QualificationInputProvider for CleanQualificationInputs {
    async fn qualification_event_for(
        &self,
        due: &DueQualificationReferral,
        observed_at_day: i64,
    ) -> Result<QualificationEvent, ReferralStoreError> {
        Ok(QualificationEvent {
            referred_id: due.referred_id.clone(),
            observed_at_day,
            deposit_minor_units: 50_000,
            kyc_tier: 1,
            completed_trade_count: 5,
            anti_abuse: serde_json::from_value(clean_anti_abuse_json())
                .expect("clean anti-abuse json should deserialize"),
            kinship_review_passed: false,
        })
    }
}

struct FixedLedgerPoster;

#[async_trait]
impl ReferralLedgerPoster for FixedLedgerPoster {
    async fn post_signup_bonus(
        &self,
        draft: SignupBonusLedgerDraftResponse,
    ) -> Result<i64, ReferralStoreError> {
        assert_eq!(draft.txn_type, "referral_signup_bonus");
        assert_eq!(draft.entries.len(), 4);
        Ok(9_901)
    }

    async fn post_commission_payout(
        &self,
        draft: CommissionPayoutLedgerDraftResponse,
    ) -> Result<i64, ReferralStoreError> {
        assert_eq!(draft.txn_type, "referral_commission_payout");
        assert_eq!(draft.entries.len(), 2);
        Ok(9_902)
    }
}

fn sample() -> AffiliateDetail {
    AffiliateDetail {
        summary: AffiliateSummary {
            id: "AFF-100".to_owned(),
            user_id: Some("user_aff_100".to_owned()),
            display_name: "Existing Partners".to_owned(),
            email_masked: "e***@partners.test".to_owned(),
            status: "active".to_owned(),
            owner_sponsor_id: None,
            commission_rate_bps: 500,
            tier: 1,
            sub_tree_size: 0,
            active_sub_count: 0,
            pending_payout_minor: 0,
            pending_payout_currency: "USDT".to_owned(),
            last_activity_at: "2026-05-15T00:00:00+08:00".to_owned(),
            risk_flags: Vec::new(),
        },
        sub_tree_breakdown: Vec::new(),
        recent_payouts: Vec::new(),
        rate_history: Vec::new(),
        audit_correlation_ids: Vec::new(),
        credit_profile: AffiliateCreditProfile {
            rating: "C".to_owned(),
            status: "active".to_owned(),
            max_active_credit_accounts: 1,
            active_credit_accounts: 0,
            total_credit_limit_minor_units: 100_000,
            per_account_limit_minor_units: 100_000,
            review_expires_at: None,
            last_changed_by: "admin-0".to_owned(),
            last_changed_at: "2026-05-15T00:00:00+08:00".to_owned(),
            audit_correlation_ids: Vec::new(),
            accounts: Vec::new(),
        },
    }
}
