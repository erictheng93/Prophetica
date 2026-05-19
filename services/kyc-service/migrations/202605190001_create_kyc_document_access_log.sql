-- SPEC-005 §5.3: KYC document access audit log.
CREATE TABLE kyc_document_access_log (
    id BIGSERIAL PRIMARY KEY,
    reviewer_id TEXT NOT NULL,
    reviewer_role TEXT NOT NULL,
    user_id TEXT NOT NULL,
    doc_id TEXT,
    action TEXT NOT NULL CHECK (action IN ('list', 'view')),
    result TEXT NOT NULL CHECK (result IN ('success', 'denied')),
    denial_reason TEXT,
    request_id TEXT,
    source_ip_hash TEXT,
    accessed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_kyc_doc_access_user
    ON kyc_document_access_log (user_id, accessed_at DESC);
