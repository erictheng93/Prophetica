export const adminLocale = "zh-Hant" as const;

// Keep operator-facing copy in one typed object so a future next-intl namespace
// can replace this module without sweeping every admin component.
export const adminCopy = {
  shell: {
    defaultEyebrow: "內部營運",
    readOnlyFacade: "唯讀操作入口",
    noDirectLedgerWrites: "不直接寫入帳本",
    navAria: "後台營運導覽",
    overview: "總覽",
    dashboard: "儀表板",
  },
  overview: {
    title: "營運操作入口",
    description:
      "會員、帳戶點數、代理權限與抽佣、KYC/風控、補償與人工審批的單一入口。此介面只負責分流與審批前檢視，敏感操作仍交由受控流程處理。",
    domainsHeading: "操作域",
    domainsDescription: "會員、點數、代理、KYC/風控、補償與人工審批工作區。",
  },
  dashboard: {
    eyebrow: "決策輔助",
    title: "決策輔助儀表板",
    description:
      "在操作入口完成分流後，用於觀察 KPI、告警、版本化報表與留痕查詢；此頁不提供任何操作或帳本寫入。",
    kpiHeading: "KPI 與告警",
    kpiDescription: "僅供觀察。營運變更仍需回到對應操作工作區與審批流程。",
    reportsHeading: "版本化報表",
    reportsDescription: "每份報表保留版本、雜湊與負責角色，支援後續稽核與重跑比對。",
    auditHeading: "留痕查詢",
    auditDescription: "預設查詢族群供 evidence lookup 使用；此頁不建立審批、不寫入帳本。",
  },
  components: {
    previewSnapshot: "預覽快照",
    open: "開啟",
    openItems: "待處理",
    workQueueHeading: "待處理工作",
    workQueueDescription: "依營運優先級排序的審查項目。",
    operationalScope: "操作範圍",
    readOnlyScope:
      "此入口為唯讀。操作者可查看脈絡、準備證據，並將決策送往受控的正式系統。",
    status: {
      new: "新進",
      reviewing: "審查中",
      blocked: "受阻",
      ready: "可審批",
    },
  },
} as const;
