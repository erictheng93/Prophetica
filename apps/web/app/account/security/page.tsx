import { redirect } from "next/navigation";
import { ProphSubNav } from "@prophetica/ui";
import {
  fetchSelfLoginEvents,
  loginEventBrowserLabel,
  loginEventDeviceLabel,
  loginEventLocationLabel,
} from "@/lib/login-events";
import { getUpstreamSessionToken, getViewer } from "@/lib/session";
import { Brand, MainNavLinks, SiteThemeToggle } from "@/lib/site-nav";

export default async function AccountSecurityPage() {
  const [viewer, upstreamSessionToken] = await Promise.all([
    getViewer(),
    getUpstreamSessionToken(),
  ]);
  if (!viewer) {
    redirect("/auth/sign-in");
  }

  const events = upstreamSessionToken
    ? await fetchSelfLoginEvents(upstreamSessionToken, { limit: 20 })
    : [];

  return (
    <main className="min-h-screen bg-proph-canvas text-proph-ink">
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={<SiteThemeToggle />}
      />
      <section className="border-b border-proph-hairline bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-10 md:px-10">
          <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary">
            Account Security
          </p>
          <h1 className="mt-3 text-[34px] font-semibold leading-[1.1] tracking-[-0.374px]">
            登入紀錄
          </h1>
          <p className="mt-3 max-w-2xl text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80">
            最近登入時間、約略位置、裝置與瀏覽器。IP 只顯示遮罩版本。
          </p>
        </div>
      </section>
      <section className="mx-auto max-w-proph px-6 py-8 md:px-10">
        <div className="overflow-hidden border border-proph-hairline bg-proph-canvas">
          <div className="hidden grid-cols-[1.3fr_1fr_1fr_1fr] border-b border-proph-hairline bg-proph-canvas-parchment px-4 py-3 text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-64 sm:grid">
            <span>時間</span>
            <span>地點</span>
            <span>裝置</span>
            <span>IP</span>
          </div>
          {events.length === 0 ? (
            <p className="px-4 py-6 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80">
              尚無可顯示的登入紀錄。
            </p>
          ) : (
            <ul className="divide-y divide-proph-hairline">
              {events.map((event) => (
                <li
                  key={event.id}
                  className="grid gap-3 px-4 py-4 text-[14px] leading-[1.43] tracking-[-0.224px] sm:grid-cols-[1.3fr_1fr_1fr_1fr]"
                >
                  <span>{formatEpoch(event.occurred_at_epoch_seconds)}</span>
                  <span>{loginEventLocationLabel(event)}</span>
                  <span>
                    {loginEventDeviceLabel(event)} / {loginEventBrowserLabel(event)}
                  </span>
                  <span>{event.ip_masked || "未知"}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>
    </main>
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
