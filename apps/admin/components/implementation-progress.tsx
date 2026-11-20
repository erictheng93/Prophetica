import type { CrossDomainStreamEvent, OpsRoute } from "@/lib/ops-facade";

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
    <article className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card">
      <h3 className="font-display text-[16px] font-semibold leading-[1.31] tracking-[-0.374px]">
        {title}
      </h3>
      <ul className="mt-4 space-y-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
        {items.map((item) => (
          <li key={item} className="flex items-start gap-2">
            <span className={`mt-1 inline-flex ${indicatorStyle}`}>
              {marker}
            </span>
            <span>{item}</span>
          </li>
        ))}
      </ul>
    </article>
  );
}

interface CrossDomainStreamProps {
  events: CrossDomainStreamEvent[];
  routes: OpsRoute[];
}

function CrossDomainStream({ events, routes }: CrossDomainStreamProps) {
  const labels = new Map(routes.map((route) => [route.domain, route.label]));

  return (
    <section aria-labelledby="cross-domain-stream-title" className="mt-6">
      <div className="mb-4">
        <h3
          id="cross-domain-stream-title"
          className="font-display text-[16px] font-semibold leading-[1.31] tracking-[-0.374px]"
        >
          跨域串流確認
        </h3>
        <p className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          已串接會員、點數、代理、KYC、補償與工單審批事件，並保留回傳確認收據。
        </p>
      </div>
      <div className="overflow-hidden rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas shadow-proph-card">
        {events.map((event) => (
          <article
            className="grid gap-3 border-b border-proph-hairline/60 bg-proph-canvas p-5 last:border-b-0 transition-colors duration-150 hover:bg-proph-canvas-parchment/60 md:grid-cols-[1fr_auto]"
            key={event.id}
          >
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-80">
                  {event.correlationId}
                </span>
                <span className="rounded-proph-pill border border-proph-feedback-success/40 bg-proph-feedback-success/10 px-2 py-0.5 font-text text-[11px] font-semibold leading-none tracking-[-0.12px] text-proph-feedback-success">
                  回傳確認
                </span>
              </div>
              <h4 className="mt-2 font-display text-[17px] font-semibold leading-[1.24] tracking-[-0.374px]">
                {event.title}
              </h4>
              <p className="mt-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {labels.get(event.sourceDomain)} -&gt;{" "}
                {event.targetDomains
                  .map((domain) => labels.get(domain) ?? domain)
                  .join(" / ")}
              </p>
              <p className="mt-1 font-mono text-[12px] leading-[1.5] tracking-[-0.12px] text-proph-ink-muted-48">
                {event.linkedQueueIds.join(" / ")}
              </p>
            </div>
            <dl className="space-y-1 text-left font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80 md:text-right">
              <div>
                <dt className="sr-only">收據</dt>
                <dd className="font-mono text-[12px] text-proph-primary">
                  {event.acknowledgement.receiptId}
                </dd>
              </div>
              <div>
                <dt className="sr-only">回傳模組</dt>
                <dd>{labels.get(event.acknowledgement.returnedFrom)}</dd>
              </div>
              <div>
                <dt className="sr-only">回傳路徑</dt>
                <dd className="font-mono text-[12px]">
                  {event.acknowledgement.returnPath}
                </dd>
              </div>
              <div>
                <dt className="sr-only">確認時間</dt>
                <dd className="font-mono text-[12px] text-proph-ink-muted-48">
                  {event.acknowledgement.confirmedAt}
                </dd>
              </div>
            </dl>
          </article>
        ))}
      </div>
    </section>
  );
}

interface ImplementationProgressProps {
  crossDomainStream: CrossDomainStreamEvent[];
  routes: OpsRoute[];
}

export function ImplementationProgress({
  crossDomainStream,
  routes,
}: ImplementationProgressProps) {
  const completedItems = [
    "完成 API 覆蓋：/api/members、/api/members/pending-cases、/api/members/actions 已串接。",
    "完成 KYC 流程拆分：會員清單 `/members` 與 KYC 待處理 `/members/kyc-review` 已獨立。",
    "完成操作入口：會員總覽、KYC 審核、代理管理與工單審批已在總覽導覽中集中呈現。",
    "完成權限閉環：RBAC + 稽核紀錄已接上 admin/manager 可執行動作、affiliate 受限於自身代理範圍。",
    "完成事件快照真實資料源：Ops Queue 已改為串接會員 pending cases/actions 與代理審批資料。",
    "完成通知鏈路：待處理與審批提醒已接上 notification-service API 與 Postgres outbox（Email + 站內訊息）。",
    "完成跨域串流：模組間事件聯動已打通，回傳確認收據會回寫到總覽串流。",
  ];

  const pendingItems: string[] = [];

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
          items={
            pendingItems.length > 0
              ? pendingItems
              : ["目前沒有剩餘總覽待補強項目。"]
          }
          indicator={pendingItems.length > 0 ? "warning" : "success"}
        />
      </div>
      <CrossDomainStream events={crossDomainStream} routes={routes} />
    </section>
  );
}
