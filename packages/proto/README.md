# packages/proto — Protocol Buffer Schema (v1)

> 對應 [SPEC-014 §6.3](../../specs/SPEC-014-tech-architecture.md) 與 [SPEC-017](../../specs/SPEC-017-monorepo-bootstrap.md) §4.4。

本目錄為 prophetica 跨服務 RPC 與事件流的**單一真相 schema**。所有 service 之 IDL（gRPC service / Kafka event message）皆於此定義，再由 `buf generate` 產出 Rust（`crates/proto/src/gen/`）與 TypeScript（`packages/proto/gen/ts/`）stub。

## 目錄

```
packages/proto/
├── v1/
│   ├── errors.proto      ErrorEnvelope + ErrorCode（全平台共用）
│   ├── common.proto      Money / Outcome / Side / OrderType / Status enums / AuditMeta
│   ├── ledger.proto      LedgerService（SPEC-002）
│   ├── engine.proto      EngineService + EngineEvent（SPEC-003）
│   ├── market.proto      MarketService + Settlement / Dispute messages（SPEC-015 / 004）
│   ├── kyc.proto         KycService（SPEC-005）
│   └── payment.proto     PaymentService（SPEC-016）
├── gen/
│   ├── go/v1/            ← buf generate 產出（commit 進 git，CI 驗證一致）
│   └── ts/v1/
├── README.md
└── (buf.yaml / buf.gen.yaml 在 monorepo 根目錄)
```

## 慣例

1. **package**：所有 message 在 `proph.v1` 命名空間。重大 breaking 走 `proph.v2`，並保留 v1 至少一個 release cycle。
2. **enum**：第 0 值永遠為 `*_UNSPECIFIED`，CI 應 reject 所有 production 路徑回傳 0。
3. **金額**：永遠用 `Money { int64 amount; string currency; }`，不用 float / decimal string。`amount` 為 SPEC-002 §3.2 定義的內部單位（× 100）。
4. **強型別 ID**：`UserID` / `MarketID` / `OrderID` 等以 wrapper message 區隔，避免 `int64` 互換錯誤。
5. **AuditMeta**：所有寫操作 RPC 第一個欄位為 `AuditMeta`，含 `idempotency_key` / `correlation_id` / `trace_id`。
6. **Errors**：所有 RPC 錯誤回應走 `ErrorEnvelope`；ErrorCode 命名 `ERROR_CODE_<DOMAIN>_<REASON>`，遵循 `errors.proto` 之區段分配。

## 開發流程

```bash
# 1. 修改 .proto
$EDITOR packages/proto/v1/ledger.proto

# 2. 在 monorepo 根目錄
make proto              # 等同於 cd packages/proto && buf generate

# 3. 提交時 CI 會驗證
buf lint
buf breaking --against '.git#branch=main'   # 對 main 不能有 breaking
```

## Breaking change 政策

- 在 `proph.v1` 內**永不**做 breaking：欄位只新增，舊欄位標記 `deprecated = true`。
- 真正需要 break 時新建 `proph.v2`，並同步更新所有 service 與客戶端，舊 v1 至少保留 1 個 release cycle。
- 若不慎 push 了 breaking change：CI 應 fail，修法是回滾或加新 enum/field 而非改舊。
