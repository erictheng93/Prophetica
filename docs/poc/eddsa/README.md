# PoC-A: EdDSA / LIFF SDK 相容性

> 對應 [SPEC-010 §3.1.2](../../../specs/SPEC-010-security-antifraud.md) + [PRE-FLIGHT-POCS §3](../PRE-FLIGHT-POCS.md)
>
> **狀態**：🟢 Desk Research Done · 🟡 Live Verification Pending（artifacts 已就緒，工程師跑完回填）

---

## 0. TL;DR

- **Desk research 結論**：5 個目標環境（iOS Safari 17+ / Android Chrome 110+ / iOS LINE LIFF webview / Android LINE LIFF webview / Cloudflare Workers）對 Ed25519 之 Web Crypto API 支援皆已成熟（2024 H2 起）。**預期通過**。
- **Live verification 重點**：LINE webview 在 LINE 13+ 各版本之 actual user-agent 行為（特別是 iOS 14 / Android 9 等舊裝置 base）。
- **執行成本**：申請 LIFF channel + 1 台 iPhone + 1 台 Android 實機 + 半天工時。
- **退路啟動門檻**：任一實機環境跑 §3 step 2 失敗 → 啟動 dual-algo（LIFF route 用 RS256，其他保 EdDSA）。

---

## 1. Desk Research 結論（2026-05-07）

### 1.1 Web Crypto API Ed25519 支援度

來源：MDN Web Crypto API compat table、Cloudflare blog、各 browser engine release notes。

| 環境 | Ed25519 in `crypto.subtle.generateKey` 支援？ | 來源 |
|------|------|------|
| **Chrome 137+** (期望 2025 H1 stable) | ✅ Yes | Chrome Status `chromestatus.com/feature/5277475032432640` |
| **Safari 17+** (2023-09) | ✅ Yes（partial）| WebKit Feature Status |
| **Safari 18+** (2024-09) | ✅ Yes（complete）| 同上 |
| **Firefox 130+** (2024-09) | ✅ Yes | Firefox release notes |
| **Cloudflare Workers** | ✅ Yes（自 2024 年中起）| Cloudflare blog "Workers crypto API extensions" |
| **Node.js 18+** | ✅ Yes | Node.js docs |

> **不確定的版本號請工程師打開 [https://caniuse.com/?search=Ed25519](https://caniuse.com/?search=Ed25519) 即時驗證**；本表為 PoC plan 撰寫日（2026-05-07）之 best-known 狀態。

### 1.2 LINE LIFF webview engine

LIFF webview 在不同平台採用之 JS engine：

| 平台 | LINE 版本 | Webview engine | Ed25519 支援預期 |
|------|---------|----------------|----------------|
| iOS | LINE 11.0+ | `WKWebView` (= host Safari engine) | iOS 17+ ✅；iOS 16 部分；iOS 15 ❌ |
| Android | LINE 11.0+ | Chrome WebView (= host Chrome version) | Chrome 137+ ✅；其他需 polyfill |

**結論**：LIFF Ed25519 支援度 ≈ host OS Safari / Chrome 版本。多數用戶升至 iOS 17+ / Android 12+，預期 ✅。但 iOS 15 / Android 9 等舊裝置 base 為已知風險點。

### 1.3 Mobile JWT lib EdDSA 支援

| Lib / Platform | Ed25519 / EdDSA 支援 |
|----------------|--------------------|
| **`jsonwebtoken` (Rust 0.9+)** | ✅ Confirmed; user-service 已採用 |
| **`jose` (npm 4+)** | ✅ Used by Node.js + browser; widely adopted |
| **Apple `CryptoKit` (iOS 13+)** | ✅ Curve25519 supported |
| **Apple `JOSESwift` 3.0+** | ✅ EdDSA added 2023; 但業界少用 |
| **Android `BouncyCastle` 1.78+** | ✅ EdDSA stable |
| **Android `Nimbus JOSE+JWT` 9.x+** | ✅ EdDSA supported |

**結論**：所有 mobile JWT lib 支援 EdDSA；但團隊應確認所選 lib 之版本鎖定。

---

## 2. Live Verification Playbook（工程師執行）

### 2.1 預備項目

- [ ] 申請 LINE Developer Console + 建立一個 LIFF channel（業主帳號）
- [ ] 1 台 iPhone（iOS 17+）+ 1 台 Android 實機（Android 12+）
- [ ] 兩部裝置安裝 LINE app 最新版
- [ ] sandbox endpoint：本機用 `ngrok http 3000` 或 Cloudflare Pages preview

### 2.2 Step 1 — Web Crypto API 環境調查（無需 LIFF channel，~30 min）

開啟 [`check-support.html`](./check-support.html) 於下列環境，記錄 console output：

1. iPhone Safari（直接打開 URL）
2. Android Chrome（直接打開 URL）
3. iPhone LINE LIFF webview（需 LIFF channel）
4. Android LINE LIFF webview（同上）
5. Cloudflare Workers（透過 wrangler dev 跑 [`server-token-sample.rs`](./server-token-sample.rs) 之 wasm-version 等同物）

**Acceptance**：每個環境輸出 `Ed25519 supported`，無 `DOMException` / `NotSupportedError`。

### 2.3 Step 2 — Server-side EdDSA token round-trip（~2 hr）

跑 [`server-token-sample.rs`](./server-token-sample.rs) 模擬 user-service token issue：

```bash
cargo run --bin eddsa-poc-server  # listens on :8888
```

從 5 個 client 環境（同 §2.2）發送 `POST /api/auth/login` → 取 cookie → 發送 `GET /api/me` 驗證。

**Acceptance 表**：

| Client | Login set-cookie OK? | Subsequent GET /me OK? | Refresh rotation OK? |
|--------|---------------------|----------------------|-------------------|
| iPhone Safari | ☐ | ☐ | ☐ |
| Android Chrome | ☐ | ☐ | ☐ |
| iPhone LINE webview | ☐ | ☐ | ☐ |
| Android LINE webview | ☐ | ☐ | ☐ |
| `apps/web` Next.js | ☐ | ☐ | ☐ |

### 2.4 退路啟動條件

任一實機環境（iPhone / Android LINE webview）`crypto.subtle` Ed25519 失敗 → 啟動 **dual-algo issuance**：

- LIFF user-agent 偵測 → server 簽 RS256 token，cookie 命名加 `_liff` 前綴
- 其他 user-agent 維持 EdDSA
- 對應 SPEC-010 §3.1.2 patch（補 dual-algo issuer）+ 新 ADR 紀錄

---

## 3. Acceptance Sign-off

PoC-A 執行完畢 → 回填 [PRE-FLIGHT-POCS §3.5](../PRE-FLIGHT-POCS.md)：

```
Status: ✅ Pass / ⚠️ Dual-algo / ❌ RS256 fallback
Executor: <工程師>
Started: <YYYY-MM-DD>
Completed: <YYYY-MM-DD>
Step 1: <result + commit SHA>
Step 2: <result + commit SHA>
Final decision: [ ] EdDSA  [ ] Dual-algo  [ ] RS256 fallback
```

若選 dual-algo / RS256 fallback → 開 SPEC-010 patch PR + 新 ADR (e.g., ADR-0007 "Dual-algorithm JWT issuance for LIFF compatibility")。
