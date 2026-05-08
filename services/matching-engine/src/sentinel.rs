//! `PLATFORM_SYSTEM_USER_IDS` 列舉所有「禁止下單」之系統 `user_id`。
//! 任何 incoming order 的 `user_id` 命中本集合則直接拒絕。
//!
//! # 紅線理由
//!   - 平台不得在自家訂單簿掛單
//!   - `ADR-0001` §6 列為靜態掃描禁字
//!
//! # 新增 `user_id` 至本集合需要
//!   1. 對應 ADR 簽核
//!   2. Compliance Officer 4-eye review（呼應 [SPEC-010 §9.3 MS-14](../../../specs/SPEC-010-security-antifraud.md)）
//!   3. 與本檔案的 git blame 對照可追溯
//!
//! # 移除 sentinel id 之 governance
//!   - 移除過之 sentinel id（如 v0.2.2 之 `uid=4 system.platform_announcement`，連動 R-2 紅線清除）
//!     **必須** 列入 [`RESERVED_REMOVED_USER_IDS`]，永不重新指派為新 sentinel
//!   - 防止歷史 ledger / WAL 中之舊 `user_id` 紀錄被誤認為新 sentinel

/// Sentinel `user_id` → 用途註記。Sorted const slice，O(log N) lookup，零 alloc。
///
/// 註：`uid=4` (`system.platform_announcement`) 已於 v0.2.2 移除（呼應
/// [SPEC-009 §5](../../../specs/SPEC-009-dmm-liquidity.md) R-2 紅線清除）。
/// 集合刻意保留 id `4` 之 gap，**不**重新編號 5/6/7/8 為 4/5/6/7，避免歷史
/// audit log 與 DB 引用斷鏈。`binary_search_by_key` 對 sorted slice with gap 仍正確運作。
pub const PLATFORM_SYSTEM_USER_IDS: &[(u64, &str)] = &[
    (1, "system.root"),       // bootstrap / migration only, never used at runtime
    (2, "system.compliance"), // 凍結 / 解凍帳戶之 audit shim, never trades
    (3, "system.settlement_oracle"), // Oracle 提交 shim, never trades
    // uid=4 reserved (formerly system.platform_announcement; 已移除，永不重用)
    (5, "system.cs_compensation"), // 客服補償帳戶，呼應 SPEC-002 §8
    (6, "system.suspense"),        // 對帳暫掛，呼應 SPEC-002 §4.2
    (7, "system.affiliate_pool"),  // 推薦獎勵發放暫存池，呼應 SPEC-011 §4
    (8, "system.dmm_rebate_pool"), // DMM 補貼暫存池，呼應 SPEC-009 §3.3
];

/// `RESERVED_REMOVED_USER_IDS` 列舉曾經存在但已正式移除之 sentinel id。
///
/// 任何嘗試重新註冊此集合中的 id（即便用途不同）必須 reject + 寫 critical audit log，
/// 避免歷史 ledger / WAL 中之 `user_id=4` 紀錄被誤認為新 sentinel。
pub const RESERVED_REMOVED_USER_IDS: &[(u64, &str)] = &[(
    4,
    "formerly system.platform_announcement; removed 2026-05-07 per R-2 (SPEC-009 §5 / ADR-0001 §6 v1.0.2)",
)];

/// Returns `true` iff the given `user_id` is reserved as a platform sentinel
/// and must **never** appear as an order placer.
#[inline]
#[must_use]
pub fn is_platform_system_user(uid: u64) -> bool {
    PLATFORM_SYSTEM_USER_IDS
        .binary_search_by_key(&uid, |&(id, _)| id)
        .is_ok()
}

/// Returns `true` iff the given `user_id` was a former sentinel that has been
/// formally removed. Useful for diagnostic / audit code paths that need to
/// distinguish "was once reserved" from "ordinary user id".
///
/// Note: ordinary order rejection MUST use [`is_platform_system_user`]; the
/// reserved-removed set is only for governance / observability purposes.
#[inline]
#[must_use]
pub fn is_reserved_removed_user(uid: u64) -> bool {
    RESERVED_REMOVED_USER_IDS
        .binary_search_by_key(&uid, |&(id, _)| id)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::{
        PLATFORM_SYSTEM_USER_IDS, RESERVED_REMOVED_USER_IDS, is_platform_system_user,
        is_reserved_removed_user,
    };

    #[test]
    fn sentinel_ids_are_sorted() {
        // binary_search_by_key 之正確性前提：必須維持 sorted ascending
        let ids: Vec<u64> = PLATFORM_SYSTEM_USER_IDS.iter().map(|&(id, _)| id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(
            ids, sorted,
            "PLATFORM_SYSTEM_USER_IDS must be sorted by user_id"
        );
    }

    #[test]
    fn sentinel_ids_are_unique() {
        use std::collections::HashSet;
        let set: HashSet<u64> = PLATFORM_SYSTEM_USER_IDS.iter().map(|&(id, _)| id).collect();
        assert_eq!(
            set.len(),
            PLATFORM_SYSTEM_USER_IDS.len(),
            "duplicate sentinel user_id"
        );
    }

    #[test]
    fn ordinary_user_is_not_sentinel() {
        assert!(!is_platform_system_user(0));
        assert!(!is_platform_system_user(42));
        assert!(!is_platform_system_user(1_000_000));
        assert!(!is_platform_system_user(u64::MAX));
    }

    #[test]
    fn all_sentinel_ids_match() {
        for &(id, _) in PLATFORM_SYSTEM_USER_IDS {
            assert!(
                is_platform_system_user(id),
                "active sentinel uid={id} must be detected by is_platform_system_user"
            );
        }
    }

    #[test]
    fn removed_uid_4_is_not_active_sentinel() {
        // R-2: system.platform_announcement (uid=4) 已於 v0.2.2 移除；不應作為 active sentinel。
        assert!(
            !is_platform_system_user(4),
            "uid=4 已於 v0.2.2 移除，不應作為 active sentinel（呼應 SPEC-009 §5 / ADR-0001 §6 v1.0.2）"
        );
    }

    #[test]
    fn removed_uid_4_is_in_reserved_removed_set() {
        // 但 uid=4 必須記錄於 RESERVED_REMOVED_USER_IDS 防止重用
        assert!(
            is_reserved_removed_user(4),
            "uid=4 必須記錄於 RESERVED_REMOVED_USER_IDS 防止重用"
        );
    }

    #[test]
    fn reserved_removed_ids_do_not_overlap_with_active() {
        let active: std::collections::HashSet<u64> =
            PLATFORM_SYSTEM_USER_IDS.iter().map(|&(id, _)| id).collect();
        for &(id, _) in RESERVED_REMOVED_USER_IDS {
            assert!(
                !active.contains(&id),
                "removed uid {id} must not reappear in active sentinel set"
            );
        }
    }

    #[test]
    fn reserved_removed_ids_are_sorted() {
        let ids: Vec<u64> = RESERVED_REMOVED_USER_IDS
            .iter()
            .map(|&(id, _)| id)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(
            ids, sorted,
            "RESERVED_REMOVED_USER_IDS must be sorted by user_id"
        );
    }

    #[test]
    fn ordinary_user_is_not_in_reserved_removed_set() {
        assert!(!is_reserved_removed_user(42));
        assert!(!is_reserved_removed_user(0));
        assert!(!is_reserved_removed_user(1_000_000));
    }
}
