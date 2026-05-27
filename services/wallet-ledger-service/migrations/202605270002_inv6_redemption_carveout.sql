-- SPEC-002 v0.1.14 §5 INV-6 / SPEC-024 §4.3 / ADR-0007 §4.1
--
-- INV-6 white-list 追加 (g) Liability.UnredeemedEntitlement.{user_id} carve-out：
-- Player.Wallet.{X} outflow 配對到 Liability.UnredeemedEntitlement.{Y} 之 credit，
-- 限：
--   - txn_type = 'redemption_purchase'
--   - metadata 必含 'order_id' / 'sku_id' / 'audit_log_id'
--
-- user_id 同人綁定（X = Y）由 INV-7 trigger 強制（見 202605270003_*）；
-- INV-6 此處只看 account-kind 合法性。
--
-- CREATE OR REPLACE FUNCTION 取代 202605070004_inv6_outflow_whitelist.sql
-- 之 fn_inv6_outflow_whitelist_check 函式體；既有 trg_inv6_outflow_whitelist
-- 仍綁同名函式，無需重建 trigger。

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
          -- (g) Player.Wallet → Liability.UnredeemedEntitlement.* 限 redemption_purchase（ADR-0007 §4.1）
          -- user_id 同人綁定由 INV-7 trigger 另外強制（fn_inv7_redemption_same_user_check）
          OR (a2.code LIKE 'Liability.UnredeemedEntitlement.%'
              AND EXISTS (
                  SELECT 1 FROM ledger_transactions t
                  WHERE t.id = NEW.txn_id
                    AND t.txn_type = 'redemption_purchase'
                    AND COALESCE(t.metadata->>'order_id', '') <> ''
                    AND COALESCE(t.metadata->>'sku_id', '') <> ''
                    AND COALESCE(t.metadata->>'audit_log_id', '') <> ''
              ))
      )
    LIMIT 1;

    IF v_violation_account IS NOT NULL THEN
        RAISE EXCEPTION
            'INV-6 violation in txn %: Player.Wallet outflow targets non-whitelisted account "%"',
            NEW.txn_id, v_violation_account
        USING ERRCODE = '40004',
              HINT = 'See ADR-0006 §4.1 / SPEC-002 §5 INV-6; ADR-0007 §4.1 (g) for redemption carve-out; SPEC-011 §6.3 for R-1 red line.';
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
