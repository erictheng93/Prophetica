interface ProgressListProps {
  title: string;
  items: string[];
  indicator: "success" | "warning";
}

function ProgressList({ title, items, indicator }: ProgressListProps) {
  const indicatorStyle =
    indicator === "success"
      ? "text-proph-feedback-success"
      : "text-proph-feedback-warning";
  const marker = indicator === "success" ? "✓" : "!";

  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5">
      <h3 className="text-[16px] font-semibold leading-[1.31] tracking-[-0.374px]">
        {title}
      </h3>
      <ul className="mt-4 space-y-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        {items.map((item) => (
          <li key={item} className="flex items-start gap-2">
            <span className={`mt-1 inline-flex ${indicatorStyle}`}>{marker}</span>
            <span>{item}</span>
          </li>
        ))}
      </ul>
    </article>
  );
}

export function ImplementationProgress() {
  const completedItems = [
    "完成 API 覆蓋：/api/members、/api/members/pending-cases、/api/members/actions 已串接。",
    "完成 KYC 流程拆分：會員清單 `/members` 與 KYC 待處理 `/members/kyc-review` 已獨立。",
    "完成操作入口：會員總覽、KYC 審核、代理管理與人工審批已在總覽導覽中集中呈現。",
    "完成權限閉環：RBAC + 稽核紀錄已接上 admin/manager 可執行動作、affiliate 受限於自身代理範圍。",
  ];

  const pendingItems = [
    "待補充事件快照真實資料源：目前 queue 仍以展示資料為主，需接上即時生產來源。",
    "待補充通知鏈路：待處理與審批提醒（郵件/訊息）尚未全量接入。",
    "待補充跨域串流：模組間事件聯動尚需打通並補齊回傳確認流程。",
  ];

  return (
    <section aria-labelledby="implementation-progress-title">
      <div className="mb-4">
        <h2
          id="implementation-progress-title"
          className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
        >
          已完成 / 待補強
        </h2>
        <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          目前版本在總覽層顯示完成與待補強清單，模組頁僅保留各自操作範圍與入口。
        </p>
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <ProgressList
          title="已完成"
          items={completedItems}
          indicator="success"
        />
        <ProgressList
          title="待補強"
          items={pendingItems}
          indicator="warning"
        />
      </div>
    </section>
  );
}
