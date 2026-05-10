export const adminLocale = "zh-Hant" as const;

// Keep operator-facing copy in one typed object so a future next-intl namespace
// can replace this module without sweeping every admin component.
export const adminCopy = {
  shell: {
    defaultEyebrow: "內部操作",
    readOnlyFacade: "唯讀操作入口",
    noDirectLedgerWrites: "不直接寫入帳本",
    navAria: "管理後台導覽",
    overview: "總覽",
    dashboard: "決策支援",
    signOut: "登出",
    roleAdmin: "管理員",
    roleManager: "主管",
    roleAffiliate: "代理",
  },
  login: {
    eyebrow: "管理後台登入",
    title: "登入 Prophetica 管理後台",
    description:
      "限平台管理員、主管與授權代理使用。所有登入動作均寫入稽核紀錄；嘗試以無效憑證登入將被風控系統標記。",
    emailLabel: "電子郵件",
    passwordLabel: "密碼",
    submit: "登入",
    submitting: "登入中…",
    invalid: "電子郵件或密碼錯誤。",
    unknownError: "登入失敗，請稍後再試。",
    devHintTitle: "開發環境憑證",
    devHintBody:
      "本地端 mock 帳號（Phase 2，密碼皆為 dev）：admin@prophetica.local / manager@prophetica.local / affiliate@prophetica.local",
    mfaPlaceholder: "MFA 將於 Phase 3 上線（SPEC-010 §9.3.3）。",
  },
  forbidden: {
    eyebrow: "存取被拒",
    title: "權限不足",
    description: "你的帳號沒有訪問此頁面的權限。如需協助，請聯絡 Ops Control。",
    backLink: "返回我的首頁",
  },
  affiliatePortal: {
    eyebrow: "代理門戶",
    title: "代理自助中心",
    description:
      "查看你自己的推薦鏈、佣金結算與待撥款進度。資料皆已脫敏；你只能看到自己代理樹下的彙總，無法查看他人個資。",
    payoutsHeading: "撥款進度",
    payoutsDescription: "目前待撥款金額與下一次結算日；歷史撥款保留 24 個月。",
    subTreeHeading: "推薦鏈",
    subTreeDescription: "三層推薦鏈下的會員彙總（人數、活躍度、本月貢獻佣金）。不含 PII。",
    placeholder: "Phase 4 將串接 referral-service。目前顯示為佔位資料。",
    affiliateIdLabel: "你的代理 ID",
  },
  overview: {
    title: "管理操作入口",
    description:
      "集中處理會員、帳戶點數、代理抽佣、KYC / 風控事件、補償與人工審批。此頁只提供操作入口與審批脈絡，實際寫入仍由核准後的系統紀錄完成。",
    domainsHeading: "操作領域",
    domainsDescription: "會員、點數例外、代理審查、KYC / 風控、補償與人工核准工作區。",
  },
  dashboard: {
    eyebrow: "決策支援",
    title: "決策支援儀表板",
    description:
      "提供管理層檢視營運 KPI、風控警示、版本化報表與稽核查詢入口。此頁維持唯讀，不提供直接人工調帳或風控處分。",
    kpiHeading: "KPI 與警示",
    kpiDescription: "彙整當前營運健康度、風險警示與審批 SLA，協助判斷是否需要升級處理。",
    reportsHeading: "版本化報表",
    reportsDescription: "列出已產生的管理報表版本、雜湊與負責團隊，方便稽核與回溯。",
    auditHeading: "稽核查詢",
    auditDescription: "提供 evidence lookup 的固定查詢名稱，讓審批、補償、代理與 KYC 事件能被追蹤。",
  },
  components: {
    previewSnapshot: "快照",
    open: "開啟",
    openItems: "未結項目",
    entryLabel: "入口",
    workQueueHeading: "待處理工作",
    workQueueDescription: "依風險、時效與責任團隊排序的操作項目。",
    operationalScope: "操作範圍",
    readOnlyScope:
      "此工作區目前是唯讀入口。任何補償、權限、KYC 或帳戶處分都必須經過對應系統與審批流程後才會寫入。",
    status: {
      new: "新項目",
      reviewing: "審查中",
      blocked: "受阻",
      ready: "可處理",
    },
  },
} as const;
