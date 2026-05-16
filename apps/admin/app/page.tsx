import { redirect } from "next/navigation";
import { AdminShell } from "@/components/admin-shell";
import { ImplementationProgress } from "@/components/implementation-progress";
import { QueueSummary } from "@/components/queue-summary";
import { RouteEntryCard } from "@/components/route-entry-card";
import { WorkQueue } from "@/components/work-queue";
import { adminCopy } from "@/lib/admin-copy";
import { getAdminOperationsSnapshot, type OpsDomainId } from "@/lib/ops-facade";
import { defaultLandingFor } from "@/lib/rbac";
import { getViewer } from "@/lib/session";

const requiredEntryDomains: OpsDomainId[] = [
  "member-management",
  "points-exceptions",
  "affiliate-review",
  "kyc-risk-events",
  "compensation-reviews",
  "manual-approvals",
];

export default async function AdminOperationsPage() {
  const viewer = await getViewer();
  if (!viewer) {
    redirect("/login");
  }
  if (viewer.role === "affiliate") {
    redirect(defaultLandingFor("affiliate"));
  }

  const snapshot = getAdminOperationsSnapshot();
  const routes = requiredEntryDomains.map((domain) => {
    const route = snapshot.routes.find((item) => item.domain === domain);

    if (!route) {
      throw new Error(`Missing admin operations route: ${domain}`);
    }

    return route;
  });

  return (
    <AdminShell
      title={adminCopy.overview.title}
      description={adminCopy.overview.description}
      role={viewer.role}
    >
      <div className="space-y-8">
        <QueueSummary summaries={snapshot.summaries} />

        <ImplementationProgress />

        <section aria-labelledby="domains-title">
          <div className="mb-4">
            <h2 id="domains-title" className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
              {adminCopy.overview.domainsHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              {adminCopy.overview.domainsDescription}
            </p>
          </div>
          <div className="grid gap-5 lg:grid-cols-3">
            {routes.map((route) => (
              <RouteEntryCard key={route.domain} route={route} role={viewer.role} />
            ))}
          </div>
        </section>

        <WorkQueue items={snapshot.queue} routes={snapshot.routes} />
      </div>
    </AdminShell>
  );
}
