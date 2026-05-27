-- SPEC-024 §3 / implementation plan §1 P2: catalog + redemption order +
-- entitlement + idempotency schema.
--
-- Lives in a redemption-service-owned namespace, NOT the wallet-ledger DB.
-- See `core-api-postgres-migration-collision` lesson: each service runs its
-- own migrator set against its own DB / schema to avoid the cross-service
-- collisions that previously broke `core-api`.

-- ---------------------------------------------------------------------------
-- Catalog
-- ---------------------------------------------------------------------------

CREATE TABLE catalog_items (
    item_id     VARCHAR(64)   PRIMARY KEY,
    kind        VARCHAR(16)   NOT NULL CHECK (kind IN ('digital', 'service')),
    -- 'physical' intentionally excluded; SPEC-024 §3.1 v1 boundary. The CHECK
    -- guarantees no row can be inserted with it even if app validation is
    -- bypassed.
    status      VARCHAR(16)   NOT NULL CHECK (status IN ('draft', 'active', 'retired')),
    category    VARCHAR(64)   NOT NULL,
    title       VARCHAR(256)  NOT NULL,
    metadata    JSONB         NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_catalog_items_status ON catalog_items(status);
CREATE INDEX idx_catalog_items_category ON catalog_items(category);

CREATE TABLE skus (
    sku_id                VARCHAR(64)  PRIMARY KEY,
    item_id               VARCHAR(64)  NOT NULL REFERENCES catalog_items(item_id),
    price_minor_units     BIGINT       NOT NULL CHECK (price_minor_units > 0),
    fulfillment_mode      VARCHAR(32)  NOT NULL CHECK (
        fulfillment_mode IN ('digital_immediate', 'digital_code', 'service_async')
    ),
    entitlement_ttl_days  INTEGER      NOT NULL CHECK (entitlement_ttl_days > 0),
    metadata              JSONB        NOT NULL DEFAULT '{}',
    created_at            TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_skus_item ON skus(item_id);

CREATE TABLE sku_stock (
    sku_id                  VARCHAR(64)  PRIMARY KEY REFERENCES skus(sku_id),
    kind                    VARCHAR(16)  NOT NULL CHECK (
        kind IN ('unlimited', 'finite', 'partner_managed')
    ),
    available_qty           BIGINT       NOT NULL DEFAULT 0,
    reservation_ttl_seconds INTEGER      NOT NULL DEFAULT 300 CHECK (reservation_ttl_seconds > 0),
    updated_at              TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Orders
-- ---------------------------------------------------------------------------

CREATE TABLE redemption_orders (
    order_id           VARCHAR(64)  PRIMARY KEY,
    user_id            VARCHAR(128) NOT NULL,
    sku_id             VARCHAR(64)  NOT NULL REFERENCES skus(sku_id),
    price_minor_units  BIGINT       NOT NULL CHECK (price_minor_units > 0),
    idempotency_key    VARCHAR(128) NOT NULL UNIQUE,
    state              VARCHAR(16)  NOT NULL CHECK (
        state IN ('submitted', 'accepted', 'committed', 'fulfilled', 'refunded', 'expired')
    ),
    -- ledger txn ids backing this order; null until the matching ledger txn
    -- commits. Populated by the redemption-service application layer, never
    -- by triggers.
    ledger_purchase_txn_id     BIGINT,
    ledger_fulfillment_txn_id  BIGINT,
    ledger_refund_txn_id       BIGINT,
    ledger_expiry_txn_id       BIGINT,
    metadata           JSONB        NOT NULL DEFAULT '{}',
    created_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    expires_at         TIMESTAMPTZ
);
CREATE INDEX idx_orders_user ON redemption_orders(user_id, created_at);
CREATE INDEX idx_orders_state ON redemption_orders(state, created_at);
CREATE INDEX idx_orders_expires ON redemption_orders(expires_at)
    WHERE expires_at IS NOT NULL AND state = 'committed';

-- ---------------------------------------------------------------------------
-- Entitlements
-- ---------------------------------------------------------------------------

CREATE TABLE entitlements (
    entitlement_id   VARCHAR(64)  PRIMARY KEY,
    order_id         VARCHAR(64)  NOT NULL UNIQUE REFERENCES redemption_orders(order_id),
    owner_user_id    VARCHAR(128) NOT NULL,
    sku_id           VARCHAR(64)  NOT NULL REFERENCES skus(sku_id),
    state            VARCHAR(16)  NOT NULL CHECK (
        state IN ('issued', 'fulfilled', 'consumed', 'refunded', 'expired')
    ),
    metadata         JSONB        NOT NULL DEFAULT '{}',
    issued_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    fulfilled_at     TIMESTAMPTZ,
    consumed_at      TIMESTAMPTZ,
    expires_at       TIMESTAMPTZ
);
CREATE INDEX idx_entitlements_owner ON entitlements(owner_user_id, state);
CREATE INDEX idx_entitlements_state_expires ON entitlements(state, expires_at);

-- ---------------------------------------------------------------------------
-- Idempotency (independent of wallet-ledger-service's idempotency_records)
-- ---------------------------------------------------------------------------

CREATE TABLE redemption_idempotency_records (
    key             VARCHAR(128) PRIMARY KEY,
    order_id        VARCHAR(64)  REFERENCES redemption_orders(order_id),
    request_hash    VARCHAR(64)  NOT NULL,
    response        JSONB        NOT NULL,
    status_code     INTEGER      NOT NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ  NOT NULL
);
CREATE INDEX idx_redemption_idempo_expires ON redemption_idempotency_records(expires_at);
