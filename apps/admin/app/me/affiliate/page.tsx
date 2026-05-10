import { AffiliateShell } from "@/components/affiliate-shell";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";

export default function AffiliateHomePage() {
  return (
    <RoleGate allow={["affiliate"]}>
      {(viewer) => (
        <AffiliateShell affiliateId={viewer.affiliateId ?? "—"} active="home">
          <div className="space-y-8">
            <section
              aria-labelledby="affiliate-summary-title"
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <h2
                id="affiliate-summary-title"
                className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]"
              >
                本月概覽
              </h2>
              <div className="mt-4 grid gap-4 md:grid-cols-3">
                <Stat label="活躍下級" value="42" detail="近 30 日有交易" />
                <Stat label="待結算佣金" value="USDT 1,284" detail="下次結算 2026-05-15" />
                <Stat label="本月新增" value="6" detail="新註冊並完成 KYC" />
              </div>
              <p className="mt-6 max-w-3xl rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {adminCopy.affiliatePortal.placeholder}
              </p>
            </section>
          </div>
        </AffiliateShell>
      )}
    </RoleGate>
  );
}

function Stat({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5">
      <h3 className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px]">{label}</h3>
      <p className="mt-3 text-[28px] font-semibold leading-[1.14] tracking-[-0.374px]">{value}</p>
      <p className="mt-2 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">{detail}</p>
    </article>
  );
}
