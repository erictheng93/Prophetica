CREATE TABLE ledger_accounts (
    id BIGSERIAL PRIMARY KEY,
    code VARCHAR(128) NOT NULL UNIQUE,
    type VARCHAR(32) NOT NULL CHECK (type IN ('asset', 'liability', 'revenue', 'expense')),
    currency VARCHAR(8) NOT NULL DEFAULT 'PTS' CHECK (currency = 'PTS'),
    parent_id BIGINT REFERENCES ledger_accounts(id),
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ
);

CREATE INDEX idx_accounts_type ON ledger_accounts(type);
CREATE INDEX idx_accounts_code ON ledger_accounts(code);

CREATE TABLE ledger_transactions (
    id BIGSERIAL PRIMARY KEY,
    txn_type VARCHAR(64) NOT NULL,
    idempotency_key VARCHAR(128) NOT NULL UNIQUE,
    correlation_id VARCHAR(128),
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    posted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_txn_type ON ledger_transactions(txn_type);
CREATE INDEX idx_txn_corr ON ledger_transactions(correlation_id);

CREATE TABLE idempotency_records (
    key VARCHAR(128) PRIMARY KEY,
    txn_id BIGINT REFERENCES ledger_transactions(id),
    response JSONB NOT NULL,
    status_code INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_idempo_expires ON idempotency_records(expires_at);

CREATE TABLE ledger_entries (
    id BIGSERIAL PRIMARY KEY,
    txn_id BIGINT NOT NULL REFERENCES ledger_transactions(id),
    account_id BIGINT NOT NULL REFERENCES ledger_accounts(id),
    direction SMALLINT NOT NULL CHECK (direction IN (1, -1)),
    amount BIGINT NOT NULL CHECK (amount > 0),
    currency VARCHAR(8) NOT NULL CHECK (currency = 'PTS'),
    balance_after BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_entries_txn ON ledger_entries(txn_id);
CREATE INDEX idx_entries_account ON ledger_entries(account_id, created_at);
CREATE INDEX idx_entries_created_at_brin ON ledger_entries USING BRIN (created_at);

CREATE OR REPLACE FUNCTION fn_ledger_balance_check()
RETURNS TRIGGER AS $$
DECLARE
    total BIGINT;
BEGIN
    SELECT COALESCE(SUM(amount * direction), 0)
    INTO total
    FROM ledger_entries
    WHERE txn_id = NEW.txn_id;

    IF total <> 0 THEN
        RAISE EXCEPTION 'Ledger imbalance for txn %: %', NEW.txn_id, total;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER trg_ledger_balance
AFTER INSERT ON ledger_entries
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION fn_ledger_balance_check();
