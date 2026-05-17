import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import {
  adminLoginDeviceLabel,
  adminLoginLocationLabel,
  fetchAdminLoginEvents,
} from "@/lib/login-events-bff";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export default function AdminLoginEventsPage() {
  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const upstreamSessionToken = await getUpstreamSessionToken();
        const events = upstreamSessionToken
          ? await fetchAdminLoginEvents(upstreamSessionToken, { limit: 50 })
          : [];

        return (
          <AdminShell
            title="登入事件查詢"
            description="查詢最近登入事件、來源 IP、約略位置、裝置與失敗原因。"
            role={viewer.role}
          >
            <section className="overflow-hidden border border-proph-hairline bg-proph-canvas">
              <div className="hidden grid-cols-[1.1fr_0.8fr_1fr_1fr_1fr_1.3fr] border-b border-proph-hairline bg-proph-canvas-parchment px-4 py-3 text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-64 lg:grid">
                <span>時間</span>
                <span>事件</span>
                <span>主體</span>
                <span>地點</span>
                <span>IP</span>
                <span>裝置</span>
              </div>
              {events.length === 0 ? (
                <p className="px-4 py-6 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80">
                  尚無可顯示的登入事件。
                </p>
              ) : (
                <ul className="divide-y divide-proph-hairline">
                  {events.map((event) => (
                    <li
                      key={event.id}
                      className="grid gap-3 px-4 py-4 text-[13px] leading-[1.38] tracking-[-0.12px] lg:grid-cols-[1.1fr_0.8fr_1fr_1fr_1fr_1.3fr]"
                    >
                      <span>{formatEpoch(event.occurred_at_epoch_seconds)}</span>
                      <span>{event.event_type}</span>
                      <span>
                        {event.principal_type}
                        {event.principal_id ? ` / ${event.principal_id}` : ""}
                      </span>
                      <span>{adminLoginLocationLabel(event)}</span>
                      <span>{event.ip_address || "未知"}</span>
                      <span>
                        {adminLoginDeviceLabel(event)}
                        {event.browser_name ? ` / ${event.browser_name}` : " / 未知"}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}

function formatEpoch(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return "未知";
  }
  return new Intl.DateTimeFormat("zh-TW", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "Asia/Taipei",
  }).format(new Date(value * 1000));
}
