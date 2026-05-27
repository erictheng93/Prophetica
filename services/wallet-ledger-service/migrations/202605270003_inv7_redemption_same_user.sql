-- SPEC-002 v0.1.14 §5 INV-7 / SPEC-024 §5 / ADR-0007 §4.3
--
-- INV-7：redemption-family txn 必須 user_id 同人綁定，禁止跨會員兌換。
--
-- 適用 txn_type：
--   - redemption_purchase    Player.Wallet.{X} → Liability.UnredeemedEntitlement.{X}
--   - entitlement_fulfillment  Liability.UnredeemedEntitlement.{X} → Revenue.VirtualGoodsSales.*
--   - redemption_refund        Liability.UnredeemedEntitlement.{X} → Player.Wallet.{X}
--   - entitlement_expiry       Liability.UnredeemedEntitlement.{X} → Revenue.EntitlementBreakage
--
-- 機制：
--   收集本 txn 內所有 Player.Wallet.{X} / Liability.UnredeemedEntitlement.{Y}
--   entries 的 user_id 字尾，去重後必須 ≤ 1 個；否則拋例外、整個 transaction
--   rollback。
--
-- 為何不靠 application：admin 後台繞過 SDK 直連 PG 可寫入跨 user 之 redemption，
-- 形式上 INV-1 / INV-6 都過。INV-7 把 user_id 綁定下沉到 DB 層，三層保護
-- （CI / SDK / DB）對齊 INV-6 之安全強度。

CREATE OR REPLACE FUNCTION fn_inv7_redemption_same_user_check()
RETURNS TRIGGER AS $$
DECLARE
    v_txn_type TEXT;
    v_user_ids TEXT[];
BEGIN
    SELECT txn_type INTO v_txn_type
    FROM ledger_transactions WHERE id = NEW.txn_id;

    IF v_txn_type NOT IN (
        'redemption_purchase',
        'entitlement_fulfillment',
        'redemption_refund',
        'entitlement_expiry'
    ) THEN
        RETURN NULL;
    END IF;

    SELECT ARRAY_AGG(DISTINCT
        regexp_replace(a.code, '^(Player\.Wallet|Liability\.UnredeemedEntitlement)\.', '')
    )
    INTO v_user_ids
    FROM ledger_entries e
    JOIN ledger_accounts a ON e.account_id = a.id
    WHERE e.txn_id = NEW.txn_id
      AND (
          a.code LIKE 'Player.Wallet.%'
          OR a.code LIKE 'Liability.UnredeemedEntitlement.%'
      );

    IF v_user_ids IS NULL OR array_length(v_user_ids, 1) IS NULL THEN
        RETURN NULL;
    END IF;

    IF array_length(v_user_ids, 1) > 1 THEN
        RAISE EXCEPTION
            'INV-7 violation in txn %: redemption-family txn must not cross user_id boundary (got: %)',
            NEW.txn_id, v_user_ids
        USING ERRCODE = '40004',
              HINT = 'See SPEC-002 §5 INV-7 / SPEC-024 §5 / ADR-0007 §4.3; SPEC-002 §4.4.7 / Q-10 prohibits peer transfer.';
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER trg_inv7_redemption_same_user
AFTER INSERT ON ledger_entries
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION fn_inv7_redemption_same_user_check();
