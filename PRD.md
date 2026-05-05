ForesightX 預測市場平台 - 產品需求文件 (PRD)
文件版本：v1.0
目標市場：亞洲 / 繁體中文市場（特定地區屏蔽）
產品定位：中心化運營、具備高併發處理能力、合規與代理機制完善的法幣/點數預測平台（Kalshi 模式）。
1. 產品概述 (Product Overview)
ForesightX 是一個讓使用者針對未來事件（如選舉、體育賽事、經濟指標）結果進行交易的預測市場平台。採用「訂單簿（Order Book）」機制，平台方負責發布市場與結算，用戶透過預測機率（價格）進行買賣。平台整合在地化金流（超商代碼/虛擬帳號），並透過完善的代理分潤機制（Affiliate System）與 LINE 社群裂變來獲取用戶。
2. 核心技術架構 (Technology Stack Summary)
基礎設施：Cloudflare (WAF/CDN/DNS) + Linode (Tokyo)
前端應用：Nuxt 3 (Vue) + TailwindCSS (部署於 Cloudflare Pages) + R2 圖床
後端與撮合引擎：Go (Golang) - 處理高併發與 WebSocket 即時推播
資料庫與快取：PostgreSQL (未來支援 Neon) + Redis (Pub/Sub & 快取) + RabbitMQ/Redis Streams (消息隊列)
第三方服務：LINE Login & LIFF API, 綠界/藍新金流, Sumsub/Jumio (自動化 KYC)
3. 使用者角色 (User Roles)
訪客 (Guest)：僅能瀏覽市場清單、當前價格（機率），無法交易。
未驗證會員 (Unverified User)：可透過 LINE 登入，能進入會員中心，但無法入金與交易。
正式會員 (Verified User)：完成自動化 KYC，可入金、掛單、撤單、提領、申請專屬推廣碼。
代理商/業務 (Affiliate/Agent)：擁有專屬後台，可查看下線結構、交易量、計算並提取佣金。
管理員 (Admin)：平台營運方，負責創建新市場、判定市場結果、審核出金、檢視風險控管數據。
4. 核心功能需求 (Core Functional Requirements)
4.1 註冊、登入與 LINE 導流 (Auth & LINE Integration)
LINE 登入與 LIFF：用戶在 LINE 群組點擊連結後，透過 LIFF 開啟「跳板中間頁」。
VPN 阻擋提示：因合規要求（強制 VPN 訪問），若偵測到受限 IP（如台灣 IP），跳板頁將不直接阻擋，而是呈現「請開啟 VPN 並使用系統預設瀏覽器（Safari/Chrome）開啟網址以繼續使用」的友善圖文教學。
帳號綁定：使用 LINE Login API 一鍵授權，並綁定用戶手機號碼或 Email 建立唯一帳號。
4.2 自動化 KYC 審核 (Automated KYC)
流程：用戶進入會員中心 ➡️ 點擊身分認證 ➡️ 透過第三方 SDK (如 Sumsub) 開啟相機。
驗證項目：上傳身分證正反面 (OCR 解析資訊) + 活體人臉檢測比對。
結果處理：通過即時開通權限；失敗則轉交後台人工審查，並寄送站內信通知。
4.3 金流系統 (Fiat Gateway & Wallet)
錢包架構：每個用戶擁有三個金額狀態：可用餘額 (Available)、鎖定餘額 (Locked/In Orders)、可提領餘額 (Withdrawable)。
入金 (Deposit)：
串接第三方金流 API。
支援：銀行虛擬帳號轉帳、超商條碼、超商代碼繳費。
收到 Webhook 回調後，自動將點數/法幣加至 可用餘額。
出金 (Withdraw)：用戶綁定本人銀行帳戶後發起提領，後台防洗錢審核後放款。
4.4 交易系統與訂單簿核心 (Trading Engine & Order Book)
定價與概率對稱機制：
合約結果僅有 YES 或 NO。
價格即機率，範圍從 0.01 到 0.99 單位。
系統強制對稱：P(Yes) + P(No) = 1.00。當某用戶掛單 0.6 YES 時，系統自動在內部等價轉換為 0.4 NO 放入訂單簿撮合。
掛單與鎖定 (Open Contract & Lock in)：
用戶提交限價單（Price & Shares）。
系統即時從 可用餘額 扣除資金，轉入 鎖定餘額。訂單狀態為 Open。
撤單 (Cancel Contract) & 空盤無損撤回：
只要訂單未被撮合，用戶可隨時點擊「撤單」。
狀態變更為 Canceled，鎖定餘額 原路退回 可用餘額，無損且無手續費。
到期未成交自動作廢：當預測事件達到「截止時間」而封盤時，系統將所有狀態為 Open 的訂單自動變更為 Expired，資金原封不動退回給用戶。
4.5 市場創建與結算 (Market Creation & Resolution)
創建新市場 (Admin Only)：
後台設定：標題、規則說明、結算標準來源（如官方網站網址）、開盤時間、截止時間、最高下注限制。
市場結算 (Resolution)：
市場結束後，管理員在後台輸入最終結果（YES 或 NO）。
系統自動將勝利方的合約以 1.00 單位結算成可用餘額，失敗方合約歸 0。
扣除系統手續費（平台抽成）。
4.6 代理與分潤機制 (Affiliate & Commission System)
分機代理與推薦碼：每個會員擁有專屬邀請連結與邀請碼。
層級結構 (Tree Structure)：支援多層級（如總代 ➡️ 一級 ➡️ 二級）。
分潤計算：
基於「下線交易產生的手續費」或「下線淨輸贏」進行抽佣（依營運策略設定）。
即時或每日凌晨 (Cron Job) 結算佣金至代理的「佣金錢包」。
代理可在前台專屬看板看見：下線活躍人數、本月貢獻佣金、推廣報表。
4.7 客服與反饋 (Support)
用戶反饋信箱：前台提供表單，用戶可提交截圖與問題，資料寫入資料庫並可發送至營運團隊 Email 或後台工單系統。
5. 非功能性需求與進階機制 (Non-Functional Requirements)
5.1 高併發與系統擴展性 (High Concurrency & Scalability)
大選/決賽流量應對：
用戶端瀏覽價格時，透過 Redis 快取讀取，不直接 query 資料庫。
WebSocket 即時推播只傳送「價格變動差異 (Delta)」，降低頻寬消耗。
掛單請求先進入 RabbitMQ / Redis Streams 進行排隊，Go 核心依序消化，避免 Postgres 資料庫發生 Deadlock 或連線數爆滿。
5.2 安全與防作弊控制 (Security & Anti-Cheat)
VPN 與訪問限制：利用 Cloudflare WAF 設定 Geo-blocking，非白名單/要求國家的 IP 無法存取 API 與前端頁面。
防對沖洗錢：同一個 IP、相同裝置指紋、或同一個 KYC 實體，禁止在同一個市場同時持有 YES 和 NO 的倉位，避免惡意洗出代理佣金。
速率限制 (Rate Limiting)：單一使用者每秒最多只能發送 N 次掛單/撤單請求（防止 API 惡意刷單攻擊）。
5.3 用戶行為追蹤 (Analytics & Tracking)
前端埋點：整合 Mixpanel 或 Google Analytics 4，追蹤「LINE 進入率」、「跳板頁流失率」、「KYC 轉換率」、「常逛但不下單的市場類型」。
後台風險看板：即時監控特定帳號的勝率，若某帳號勝率異常高且資金龐大，系統需發出「Smart Money / 內線風險」警報供管理員查核。
6. 專案開發階段建議 (Milestones)
階段一：MVP 最小可行性產品 (預計耗時：2-3 個月)
目標：完成核心交易閉環與驗證市場水溫。
實作重點：
前端基礎 UI、會員註冊與 LINE 登入。
Go 後端訂單簿引擎（支援限價單、概率對稱、撮合邏輯）。
串接第三方金流入金。
手動結算系統與空盤自動退款機制。
階段二：商業化與裂變增長 (預計耗時：1.5-2 個月)
目標：上線推廣，完善合規與業務擴張。
實作重點：
上線自動化 KYC 系統 (Sumsub)。
代理分潤系統（生成連結、層級計算、分潤數據看板）。
Cloudflare Geo-blocking 與 LINE 跳板頁優化。
用戶反饋工單系統。
階段三：高可用性與進階營運 (持續迭代)
實作重點：
將 Go API 伺服器容器化 (Docker/K8s)，以便在大型賽事期間自動橫向擴展 (Auto-scaling)。
引入造市商 (Market Maker) 機器人 API，解決初期冷門市場空盤問題。
完善後台行為追蹤與反洗錢 (AML) 風控警報。
開發團隊注意事項：
本專案涉及高度精確的浮點數運算（小數點後兩位至四位），資料庫中所有金額與價格欄位請務必使用 DECIMAL 或 NUMERIC 型別，絕對不可使用 FLOAT，以免產生精度丟失。
因為採用 Go 開發撮合引擎，資料庫交易 (Database Transactions) 必須嚴格遵循 ACID 原則，確保高併發下的扣款與合約派發不會發生超賣 (Overselling)。