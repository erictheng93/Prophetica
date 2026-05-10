import { AdminShell } from "@/components/admin-shell";
import { QueueSummary } from "@/components/queue-summary";
import { RouteEntryCard } from "@/components/route-entry-card";
import { WorkQueue } from "@/components/work-queue";
import { adminCopy } from "@/lib/admin-copy";
import { getAdminOperationsSnapshot, type OpsDomainId } from "@/lib/ops-facade";

const requiredEntryDomains: OpsDomainId[] = [
  "member-management",
  "points-exceptions",
  "affiliate-review",
  "kyc-risk-events",
  "compensation-reviews",
  "manual-approvals",
];

export default function AdminOperationsPage() {
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
    >
      <div className="space-y-8">
        <QueueSummary summaries={snapshot.summaries} />

        <section aria-labelledby="domains-title">
          <div className="mb-3">
            <h2 id="domains-title" className="text-[22px] font-semibold leading-7">
              {adminCopy.overview.domainsHeading}
            </h2>
            <p className="mt-1 text-[14px] leading-5 text-proph-ink-muted-80">
              {adminCopy.overview.domainsDescription}
            </p>
          </div>
          <div className="grid gap-4 lg:grid-cols-3">
            {routes.map((route) => (
              <RouteEntryCard key={route.domain} route={route} />
            ))}
          </div>
        </section>

        <WorkQueue items={snapshot.queue} routes={snapshot.routes} />
      </div>
    </AdminShell>
  );
}
