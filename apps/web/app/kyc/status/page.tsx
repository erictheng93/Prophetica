import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import type { Icon } from "@phosphor-icons/react";
import {
  CheckCircle,
  Circle,
  Spinner,
  XCircle,
} from "@phosphor-icons/react/dist/ssr";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";
import { fetchKycOverview, type KycOverview } from "@/lib/kyc-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.status.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

type StepState = "done" | "current" | "pending" | "failed";

interface TimelineStep {
  key: "tier1" | "tier2" | "sumsub" | "notification" | "tierUpdate";
  state: StepState;
  timestamp?: string;
}

const stateColor: Record<StepState, string> = {
  done: "border-proph-ink-muted-80 bg-proph-canvas",
  current: "border-proph-primary bg-proph-canvas-parchment",
  pending: "border-proph-divider-soft bg-proph-canvas",
  failed: "border-proph-feedback-error bg-proph-canvas",
};

const stateIcon: Record<StepState, Icon> = {
  done: CheckCircle,
  current: Spinner,
  pending: Circle,
  failed: XCircle,
};

const stateIconClass: Record<StepState, string> = {
  done: "text-proph-feedback-success",
  current: "text-proph-primary animate-spin",
  pending: "text-proph-ink-muted-48",
  failed: "text-proph-feedback-error",
};

const nextStepKeys = ["markets", "disputes"] as const;

type StatusCopy = Awaited<ReturnType<typeof getTranslations<"kyc.status">>>;

function Timeline({
  t,
  timeline,
}: {
  t: StatusCopy;
  timeline: TimelineStep[];
}) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">
          {t("timeline.heading")}
        </h2>

        <ol className="space-y-3">
          {timeline.map((step, idx) => {
            const Icon = stateIcon[step.state];
            return (
              <li
                key={`${step.key}-${idx}`}
                className={`rounded-proph-lg border-l-4 border bg-proph-canvas p-5 ${stateColor[step.state]}`}
              >
                <div className="flex flex-wrap items-baseline justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <Icon
                      weight="bold"
                      className={`h-5 w-5 shrink-0 ${stateIconClass[step.state]}`}
                      aria-hidden
                    />
                    <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                      {t("timeline.stepLabel", {
                        step: idx + 1,
                        state: t(`timeline.states.${step.state}`),
                      })}
                    </span>
                    <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                      {t(`timeline.steps.${step.key}.label`)}
                    </h3>
                  </div>
                  {step.timestamp ? (
                    <span className="font-mono text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
                      {step.timestamp}
                    </span>
                  ) : null}
                </div>
                <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                  {t(`timeline.steps.${step.key}.detail`)}
                </p>
              </li>
            );
          })}
        </ol>
      </div>
    </section>
  );
}

function NextSteps({ t }: { t: StatusCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-6 font-display text-[40px] font-semibold leading-[1.1]">
          {t("next.heading")}
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {nextStepKeys.map((key) => (
            <article
              key={key}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                {t(`next.items.${key}.title`)}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`next.items.${key}.body`)}
              </p>
              <ProphLinkButton
                variant="secondary-pill"
                href={key === "markets" ? "/markets" : "/support/disputes"}
                className="mt-4"
              >
                {t(`next.items.${key}.cta`)}
              </ProphLinkButton>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

export default async function StatusPage() {
  const [t, overview] = await Promise.all([
    getTranslations("kyc.status"),
    fetchKycOverview(),
  ]);
  const timeline = buildTimeline(overview);

  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <Timeline t={t} timeline={timeline} />
      <NextSteps t={t} />
    </>
  );
}

function buildTimeline(overview: KycOverview): TimelineStep[] {
  const tier1Done = overview.currentTierNumber >= 1;
  const tier2Done = overview.currentTierNumber >= 2 && overview.status === "passed";
  const tier2Current =
    overview.currentTier === "TIER_2" && overview.status === "pending";
  const failed = overview.status === "rejected" || overview.status === "frozen";

  return [
    {
      key: "tier1",
      state: failed && !tier1Done ? "failed" : tier1Done ? "done" : "pending",
      timestamp: tier1Done ? overview.submittedAt ?? undefined : undefined,
    },
    {
      key: "tier2",
      state: tier2Done ? "done" : tier2Current ? "current" : failed ? "failed" : "pending",
      timestamp: overview.currentTierNumber >= 2 ? overview.submittedAt ?? undefined : undefined,
    },
    {
      key: "sumsub",
      state: tier2Done ? "done" : tier2Current ? "current" : failed ? "failed" : "pending",
      timestamp: overview.provider === "sumsub" ? overview.submittedAt ?? undefined : undefined,
    },
    {
      key: "notification",
      state: overview.reviewedAt ? "done" : tier2Current ? "pending" : failed ? "failed" : "pending",
      timestamp: overview.reviewedAt ?? undefined,
    },
    {
      key: "tierUpdate",
      state: overview.status === "passed" ? "done" : tier2Current ? "pending" : failed ? "failed" : "pending",
      timestamp: overview.reviewedAt ?? undefined,
    },
  ];
}
