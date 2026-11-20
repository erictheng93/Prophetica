import { AdminShell } from "@/components/admin-shell";
import { AffiliateCreateForm } from "@/components/affiliate-admin-forms";
import { AffiliateList } from "@/components/affiliate-list";
import { RoleGate } from "@/components/role-gate";
import { adminCopy } from "@/lib/admin-copy";
import {
  fetchAdminAffiliateList,
  fetchAffiliateAccountSaga,
} from "@/lib/affiliate-management-bff";
import { getOpsRoute } from "@/lib/ops-facade";

export default function AffiliatePage({
  searchParams,
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const route = getOpsRoute("affiliate-review");

  return (
    <RoleGate allow={["admin", "manager"]}>
      {async (viewer) => {
        const params = (await searchParams) ?? {};
        const sagaKey = firstParam(params.affiliateAccountSaga);
        const affiliates = await fetchAdminAffiliateList(viewer);
        const saga = sagaKey
          ? await fetchAffiliateAccountSaga(viewer, sagaKey)
          : null;
        return (
          <AdminShell
            eyebrow={route.label}
            title={adminCopy.affiliateAdmin.listHeading}
            description={adminCopy.affiliateAdmin.listDescription}
          >
            <div className="space-y-8">
              {saga && (
                <section className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card md:p-8">
                  <h2 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                    建立狀態
                  </h2>
                  <dl className="mt-4 grid gap-3 text-[14px] md:grid-cols-4">
                    <StatusItem label="Saga Key" value={saga.idempotencyKey} />
                    <StatusItem label="狀態" value={saga.status} />
                    <StatusItem label="代理 ID" value={saga.affiliateId} />
                    <StatusItem label="User ID" value={saga.targetUserId} />
                  </dl>
                  {saga.lastError && (
                    <p className="mt-3 text-[13px] leading-[1.38] text-proph-feedback-error">
                      {saga.lastError}
                    </p>
                  )}
                </section>
              )}
              <AffiliateCreateForm role={viewer.role} />
              <AffiliateList affiliates={affiliates} />
            </div>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}

function firstParam(value: string | string[] | undefined): string | null {
  if (Array.isArray(value)) return value[0] ?? null;
  return value ?? null;
}

function StatusItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
        {label}
      </dt>
      <dd className="mt-1 break-all text-proph-ink">{value}</dd>
    </div>
  );
}
