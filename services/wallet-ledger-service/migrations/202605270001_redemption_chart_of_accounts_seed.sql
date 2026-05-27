-- SPEC-002 v0.1.14 §4.2 / SPEC-024 §4.1 / ADR-0007 §4
--
-- 為 SPEC-024 redemption / entitlement 流程預備 3 個聚合科目：
--   Revenue.VirtualGoodsSales.digital   數位類虛擬商品銷貨收入
--   Revenue.VirtualGoodsSales.service   服務類兌換銷貨收入
--   Revenue.EntitlementBreakage         未消費過期 entitlement 之 breakage 收入
--
-- per-user 子科目 Liability.UnredeemedEntitlement.{user_id} 採 lazy creation：
-- 第一筆 redemption_purchase 寫入時由 service 層在同 transaction 內先 INSERT
-- 帳戶再 INSERT entries（同 Player.Wallet.{user_id} 既有 pattern），所以本
-- migration 不預先 INSERT per-user 行。
--
-- currency CHECK (= 'PTS') 已由 202605050001_create_ledger.sql 之 CHECK 約束。

INSERT INTO ledger_accounts (code, type, currency, metadata)
VALUES
    (
        'Revenue.VirtualGoodsSales.digital',
        'revenue',
        'PTS',
        '{"doc_ref": "SPEC-024 §4.1", "sku_kind": "digital"}'::jsonb
    ),
    (
        'Revenue.VirtualGoodsSales.service',
        'revenue',
        'PTS',
        '{"doc_ref": "SPEC-024 §4.1", "sku_kind": "service"}'::jsonb
    ),
    (
        'Revenue.EntitlementBreakage',
        'revenue',
        'PTS',
        '{"doc_ref": "SPEC-024 §4.1 / §7", "purpose": "unused_entitlement_breakage"}'::jsonb
    )
ON CONFLICT (code) DO NOTHING;
