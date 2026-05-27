-- SPEC-002 v0.1.14 §5 INV-6 / SPEC-024 §4.3 / ADR-0007 v1.1
--
-- Codex review (P2 follow-up): the INV-6 (g) carve-out introduced by
-- 202605270002_inv6_redemption_carveout.sql only required `order_id`,
-- `sku_id`, and `audit_log_id` to be non-empty strings. A malicious or
-- buggy caller could satisfy the carve-out with dummy "x" / "y" / "z"
-- strings and slip a Player.Wallet outflow into
-- Liability.UnredeemedEntitlement that does not correspond to any real
-- redemption order. This migration tightens the trigger to require
-- shape-realistic identifiers:
--
--   - order_id      : must look like a ULID (26-char Crockford base32)
--   - sku_id        : ≥ 3 chars, [A-Za-z0-9_-]
--   - audit_log_id  : ≥ 8 chars, [A-Za-z0-9_:-]
--
-- These checks are defense-in-depth, not perfect proof of existence —
-- a cross-DB FK to the redemption-service schema is not feasible.
-- They raise the bar from "type anything non-empty" to "construct
-- valid-looking IDs", which catches accidents and most casual misuse
-- while keeping legitimate redemption-service writes flowing.
--
-- Cross-layer mirror: SDK is_inv6_whitelisted_target() in
-- crates/sdk/src/ledger/mod.rs currently does NOT see ledger txn
-- metadata, so format validation lives only in the DB trigger. SDK
-- only checks account-kind shape and per-txn user_id binding (INV-7).
-- This asymmetry is acknowledged in ADR-0007 §4.4 v1.1 and is the
-- documented boundary until the SDK gains a redemption-metadata draft API.

CREATE OR REPLACE FUNCTION fn_inv6_outflow_whitelist_check()
RETURNS TRIGGER AS $$
DECLARE
    v_account_code TEXT;
    v_violation_account TEXT;
BEGIN
    SELECT code INTO v_account_code
    FROM ledger_accounts WHERE id = NEW.account_id;

    IF v_account_code IS NULL
       OR v_account_code NOT LIKE 'Player.Wallet.%'
       OR NEW.direction <> 1 THEN
        RETURN NULL;
    END IF;

    SELECT a2.code INTO v_violation_account
    FROM ledger_entries e2
    JOIN ledger_accounts a2 ON e2.account_id = a2.id
    WHERE e2.txn_id = NEW.txn_id
      AND e2.direction = -1
      AND NOT (
          a2.code LIKE 'Player.Locked.%'
          OR a2.code LIKE 'Player.Frozen.%'
          OR a2.code = 'Revenue.TradingFee'
          OR a2.code = 'Revenue.SettlementFee'
          -- Player.Wallet → Player.Wallet 限 trade / settle
          OR (a2.code LIKE 'Player.Wallet.%'
              AND EXISTS (
                  SELECT 1 FROM ledger_transactions t
                  WHERE t.id = NEW.txn_id
                    AND t.txn_type IN ('trade', 'settle')
              ))
          -- Player.Wallet → Bank.TrustAccount.* 限 chargeback reversal（SPEC-002 §8.1）
          OR (a2.code LIKE 'Bank.TrustAccount.%'
              AND EXISTS (
                  SELECT 1 FROM ledger_transactions t
                  WHERE t.id = NEW.txn_id
                    AND t.txn_type = 'chargeback'
              ))
          -- Player.Wallet → Expense.ManualPointsGrant 限 MVP 手動扣點沖回（SPEC-002 v0.1.9）
          OR (a2.code = 'Expense.ManualPointsGrant'
              AND EXISTS (
                  SELECT 1 FROM ledger_transactions t
                  WHERE t.id = NEW.txn_id
                    AND t.txn_type = 'manual_points_adjustment'
                    AND t.metadata->>'adjustment_direction' = 'deduct'
                    AND t.metadata->>'actor_role' IN ('admin', 'manager')
                    AND COALESCE(t.metadata->>'audit_log_id', '') <> ''
              ))
          -- (g) Player.Wallet → Liability.UnredeemedEntitlement.* 限 redemption_purchase
          --     metadata 須通過 shape-realistic 檢查（ADR-0007 §4.1 + v1.1 format tightening）
          OR (a2.code LIKE 'Liability.UnredeemedEntitlement.%'
              AND EXISTS (
                  SELECT 1 FROM ledger_transactions t
                  WHERE t.id = NEW.txn_id
                    AND t.txn_type = 'redemption_purchase'
                    AND COALESCE(t.metadata->>'order_id', '')
                        ~ '^[0-9A-HJKMNP-TV-Z]{26}$'
                    AND COALESCE(t.metadata->>'sku_id', '')
                        ~ '^[A-Za-z0-9_-]{3,64}$'
                    AND COALESCE(t.metadata->>'audit_log_id', '')
                        ~ '^[A-Za-z0-9_:-]{8,160}$'
              ))
      )
    LIMIT 1;

    IF v_violation_account IS NOT NULL THEN
        RAISE EXCEPTION
            'INV-6 violation in txn %: Player.Wallet outflow targets non-whitelisted account "%"',
            NEW.txn_id, v_violation_account
        USING ERRCODE = '40004',
              HINT = 'See ADR-0006 §4.1 / SPEC-002 §5 INV-6; ADR-0007 §4.1 (g) for redemption carve-out (v1.1 added metadata format checks).';
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
