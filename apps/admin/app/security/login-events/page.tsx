import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import {
  adminLoginDeviceLabel,
  adminLoginLocationLabel,
  fetchAdminLoginEvents,
} from "@/lib/login-events-bff";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export default function AdminLoginEventsPage({
  searchParams,
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  return (
    <RoleGate allow={["admin", "manager"]}>
      {async () => {
        const params = await searchParams;
        const upstreamSessionToken = await getUpstreamSessionToken();
        const events = upstreamSessionToken
          ? await fetchAdminLoginEvents(upstreamSessionToken, {
              limit: Number(firstParam(params?.limit) ?? 50),
              filters: {
                principal_type: firstParam(params?.principal_type),
                principal_id: firstParam(params?.principal_id),
                user_id: firstParam(params?.user_id),
                ip_address: firstParam(params?.ip_address),
                event_type: firstParam(params?.event_type),
                from_epoch_seconds: firstParam(params?.from_epoch_seconds),
                to_epoch_seconds: firstParam(params?.to_epoch_seconds),
              },
            })
          : [];

        return (
          <AdminShell
            title="登入事件查詢"
            description="查詢最近登入事件、來源 IP、約略位置、裝置與失敗原因。"
          >
            <form
              action="/security/login-events"
              className="mb-6 grid gap-3 rounded-proph-sm border border-proph-hairline bg-proph-canvas-parchment p-4 md:grid-cols-4"
            >
              <select
                name="principal_type"
                defaultValue={firstParam(params?.principal_type) ?? ""}
                className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]"
              >
                <option value="">全部主體</option>
                <option value="user">user</option>
                <option value="admin">admin</option>
                <option value="manager">manager</option>
                <option value="affiliate">affiliate</option>
              </select>
              <select
                name="event_type"
                defaultValue={firstParam(params?.event_type) ?? ""}
                className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]"
              >
                <option value="">全部事件</option>
                <option value="login_success">login_success</option>
                <option value="login_failure">login_failure</option>
              </select>
              <input
                name="principal_id"
                defaultValue={firstParam(params?.principal_id)}
                placeholder="principal id"
                className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]"
              />
              <input
                name="ip_address"
                defaultValue={firstParam(params?.ip_address)}
                placeholder="IP"
                className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]"
              />
              <input
                name="user_id"
                defaultValue={firstParam(params?.user_id)}
                placeholder="user id"
                className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]"
              />
              <input
                name="from_epoch_seconds"
                defaultValue={firstParam(params?.from_epoch_seconds)}
                placeholder="from epoch"
                inputMode="numeric"
                className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]"
              />
              <input
                name="to_epoch_seconds"
                defaultValue={firstParam(params?.to_epoch_seconds)}
                placeholder="to epoch"
                inputMode="numeric"
                className="h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]"
              />
              <button
                type="submit"
                className="h-11 rounded-proph-sm bg-proph-primary px-4 text-[14px] font-semibold text-proph-on-primary"
              >
                查詢
              </button>
            </form>
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

function firstParam(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
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
