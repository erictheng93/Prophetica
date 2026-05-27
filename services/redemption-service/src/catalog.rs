//! Catalog domain: items, SKUs, stock.
//!
//! SPEC-024 §3.1. P2 boundary: pure types + minimal invariants. Persistence,
//! activation gating, and merchandising live in later phases.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Catalog kind. SPEC-024 §3.1 v1 limits production to `Digital` and
/// `Service`; `Physical` is reserved and rejected by [`CatalogItem::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogItemKind {
    Digital,
    Service,
    /// Reserved — physical fulfillment is not in SPEC-024 v0.1 scope.
    Physical,
}

/// Whether the catalog item is currently visible/purchasable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogItemStatus {
    Draft,
    Active,
    Retired,
}

/// SPEC-024 §3.1 `CatalogItem`. One item may map to multiple SKUs (e.g. size /
/// duration variants); pricing and stock live on the SKU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogItem {
    pub item_id: String,
    pub kind: CatalogItemKind,
    pub status: CatalogItemStatus,
    pub category: String,
    pub title: String,
}

impl CatalogItem {
    /// Constructs a draft catalog item. Rejects `Physical` per SPEC-024 §3.1
    /// v1 boundary.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::PhysicalNotSupported`] if `kind == Physical`,
    /// or [`CatalogError::EmptyField`] if any required string field is empty.
    pub fn new(
        item_id: impl Into<String>,
        kind: CatalogItemKind,
        category: impl Into<String>,
        title: impl Into<String>,
    ) -> Result<Self, CatalogError> {
        if matches!(kind, CatalogItemKind::Physical) {
            return Err(CatalogError::PhysicalNotSupported);
        }
        let item_id = item_id.into();
        let category = category.into();
        let title = title.into();
        if item_id.is_empty() {
            return Err(CatalogError::EmptyField { field: "item_id" });
        }
        if category.is_empty() {
            return Err(CatalogError::EmptyField { field: "category" });
        }
        if title.is_empty() {
            return Err(CatalogError::EmptyField { field: "title" });
        }
        Ok(Self {
            item_id,
            kind,
            status: CatalogItemStatus::Draft,
            category,
            title,
        })
    }
}

/// How a redeemed SKU is delivered. SPEC-024 §3.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentMode {
    /// Delivered inline with commit; entitlement transitions straight to
    /// `Fulfilled`.
    DigitalImmediate,
    /// Code drawn from a vault; entitlement holds the code.
    DigitalCode,
    /// Worker / partner-driven; entitlement stays `Issued` until worker reports.
    ServiceAsync,
}

/// SPEC-024 §3.1 `SKU`. Smallest redeemable unit; price and stock attach here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sku {
    pub sku_id: String,
    pub item_id: String,
    /// Price in SPEC-002 §3.2 internal minor units (= points × 100).
    pub price_minor_units: i64,
    pub fulfillment_mode: FulfillmentMode,
    /// Days an issued entitlement remains valid before expiry sweep
    /// (SPEC-024 §3.3 / §7).
    pub entitlement_ttl_days: u32,
}

impl Sku {
    /// Constructs an SKU. Rejects non-positive prices or zero TTL.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::EmptyField`] on empty IDs;
    /// [`CatalogError::InvalidPrice`] when `price_minor_units <= 0`;
    /// [`CatalogError::InvalidTtl`] when `entitlement_ttl_days == 0`.
    pub fn new(
        sku_id: impl Into<String>,
        item_id: impl Into<String>,
        price_minor_units: i64,
        fulfillment_mode: FulfillmentMode,
        entitlement_ttl_days: u32,
    ) -> Result<Self, CatalogError> {
        let sku_id = sku_id.into();
        let item_id = item_id.into();
        if sku_id.is_empty() {
            return Err(CatalogError::EmptyField { field: "sku_id" });
        }
        if item_id.is_empty() {
            return Err(CatalogError::EmptyField { field: "item_id" });
        }
        if price_minor_units <= 0 {
            return Err(CatalogError::InvalidPrice { price_minor_units });
        }
        if entitlement_ttl_days == 0 {
            return Err(CatalogError::InvalidTtl);
        }
        Ok(Self {
            sku_id,
            item_id,
            price_minor_units,
            fulfillment_mode,
            entitlement_ttl_days,
        })
    }
}

/// Stock semantics. SPEC-024 §3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockKind {
    /// No cap — typical for purely digital items (badges, emotes).
    Unlimited,
    /// Hard cap; reservation consumes a unit.
    Finite,
    /// Inventory tracked externally; reservation calls partner adapter.
    PartnerManaged,
}

/// SPEC-024 §3.1 `Stock`. For `Unlimited`, `available_qty` is unused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stock {
    pub sku_id: String,
    pub kind: StockKind,
    pub available_qty: i64,
    pub reservation_ttl_seconds: u32,
}

impl Stock {
    /// True if a reservation of `qty` units is currently feasible. Caller is
    /// responsible for atomic decrement (out of P2 scope).
    #[must_use]
    pub fn can_reserve(&self, qty: i64) -> bool {
        match self.kind {
            StockKind::Unlimited => qty > 0,
            // Partner stock requires an external check; conservatively assume
            // the local snapshot is authoritative for the test surface.
            StockKind::Finite | StockKind::PartnerManaged => qty > 0 && self.available_qty >= qty,
        }
    }
}

/// Catalog errors raised by constructors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CatalogError {
    #[error("required catalog field {field} is empty")]
    EmptyField { field: &'static str },
    #[error("SPEC-024 §3.1 v1: physical SKUs are not enabled")]
    PhysicalNotSupported,
    #[error("SKU price must be positive (got {price_minor_units})")]
    InvalidPrice { price_minor_units: i64 },
    #[error("entitlement TTL must be at least 1 day")]
    InvalidTtl,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{
        CatalogError, CatalogItem, CatalogItemKind, CatalogItemStatus, FulfillmentMode, Sku, Stock,
        StockKind,
    };

    #[test]
    fn catalog_item_is_created_as_draft() {
        let item = CatalogItem::new(
            "badge_001",
            CatalogItemKind::Digital,
            "cosmetic",
            "Gold Badge",
        )
        .expect("draft");
        assert_eq!(item.status, CatalogItemStatus::Draft);
    }

    #[test]
    fn catalog_item_rejects_physical_kind() {
        let error = CatalogItem::new("phys_001", CatalogItemKind::Physical, "merch", "T-Shirt")
            .expect_err("physical disabled");
        assert_eq!(error, CatalogError::PhysicalNotSupported);
    }

    #[test]
    fn catalog_item_rejects_empty_required_fields() {
        let error =
            CatalogItem::new("", CatalogItemKind::Digital, "cat", "title").expect_err("empty");
        assert!(matches!(
            error,
            CatalogError::EmptyField { field: "item_id" }
        ));
    }

    #[test]
    fn sku_rejects_zero_and_negative_price() {
        let error = Sku::new("sku_x", "item_x", 0, FulfillmentMode::DigitalImmediate, 30)
            .expect_err("zero");
        assert!(matches!(error, CatalogError::InvalidPrice { .. }));
        let error = Sku::new("sku_x", "item_x", -1, FulfillmentMode::DigitalImmediate, 30)
            .expect_err("neg");
        assert!(matches!(error, CatalogError::InvalidPrice { .. }));
    }

    #[test]
    fn sku_rejects_zero_ttl() {
        let error = Sku::new("sku_x", "item_x", 100, FulfillmentMode::DigitalImmediate, 0)
            .expect_err("zero ttl");
        assert_eq!(error, CatalogError::InvalidTtl);
    }

    #[test]
    fn unlimited_stock_can_reserve_positive_qty() {
        let stock = Stock {
            sku_id: "sku_x".to_owned(),
            kind: StockKind::Unlimited,
            available_qty: 0,
            reservation_ttl_seconds: 300,
        };
        assert!(stock.can_reserve(1));
        assert!(!stock.can_reserve(0));
        assert!(!stock.can_reserve(-1));
    }

    #[test]
    fn finite_stock_respects_available_qty() {
        let stock = Stock {
            sku_id: "sku_x".to_owned(),
            kind: StockKind::Finite,
            available_qty: 3,
            reservation_ttl_seconds: 300,
        };
        assert!(stock.can_reserve(1));
        assert!(stock.can_reserve(3));
        assert!(!stock.can_reserve(4));
    }
}
