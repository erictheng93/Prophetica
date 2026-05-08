-- SPEC-002 §5 INV-6 / ADR-0006 §4.2.1: Player.Wallet.* outflow whitelist
--
-- 紅線：玩家錢包點數之 outflow（debit, direction = +1，債務減少）目的地僅限白名單；
-- 禁止流向 Liability.AccountsPayable.* / Bank.OperatingAccount.* / Expense.AffiliatePartnerFee
-- 等任何「兌現」科目。連動 [SPEC-011 §6.3 R-1 紅線清除]。
--
-- Whitelist:
--   - Player.Locked.{user_id}     鎖倉
--   - Player.Frozen.{user_id}     凍結（自我排除 / Compliance / EDD）
--   - Revenue.TradingFee          交易手續費
--   - Revenue.SettlementFee       結算手續費
--   - Player.Wallet.{other_user_id}  限 trade settlement（呼應 SPEC-002 §4.4.4）
--
-- 任何違反 → DB CONSTRAINT TRIGGER 拋例外 + transaction rollback；連動 SPEC-007 §7.2 P0 告警。

CREATE OR REPLACE FUNCTION fn_inv6_outflow_whitelist_check()
RETURNS TRIGGER AS $$
DECLARE
    v_account_code TEXT;
    v_violation_account TEXT;
BEGIN
    -- 取本 entry 對應之 account code
    SELECT code INTO v_account_code
    FROM ledger_accounts WHERE id = NEW.account_id;

    -- INV-6 僅檢查「Player.Wallet.* 之 outflow（direction = +1）」
    IF v_account_code IS NULL
       OR v_account_code NOT LIKE 'Player.Wallet.%'
       OR NEW.direction <> 1 THEN
        RETURN NULL;
    END IF;

    -- 檢查同 transaction 內所有 credit 配對方（direction = -1）之目的地是否在白名單內
    -- 白名單對齊 ADR-0006 §4.1 / SPEC-002 §5 INV-6 / crates/sdk/src/ledger/mod.rs::is_inv6_whitelisted_target
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
          -- 此非「點數變現」，是 deposit 反向沖正；玩家不取得現金。
          OR (a2.code LIKE 'Bank.TrustAccount.%'
              AND EXISTS (
                  SELECT 1 FROM ledger_transactions t
                  WHERE t.id = NEW.txn_id
                    AND t.txn_type = 'chargeback'
              ))
      )
    LIMIT 1;

    IF v_violation_account IS NOT NULL THEN
        RAISE EXCEPTION
            'INV-6 violation in txn %: Player.Wallet outflow targets non-whitelisted account "%"',
            NEW.txn_id, v_violation_account
        USING ERRCODE = '40004',
              HINT = 'See ADR-0006 §4.1 / SPEC-002 §5 INV-6; SPEC-011 §6.3 for R-1 red line.';
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- DEFERRABLE INITIALLY DEFERRED：與 INV-1 trigger 一致，於 commit 時集中檢查；
-- 違反時整個 transaction rollback，而非只 reject 該筆 entry。
CREATE CONSTRAINT TRIGGER trg_inv6_outflow_whitelist
AFTER INSERT ON ledger_entries
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION fn_inv6_outflow_whitelist_check();
