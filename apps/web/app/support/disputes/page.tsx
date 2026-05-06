import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ArrowUpRight, Warning } from "@phosphor-icons/react/dist/ssr";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("support.disputes.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

type Priority = "urgent" | "high" | "standard";

interface Category {
  id: "settlement" | "kyc" | "topup" | "security" | "technical" | "rg";
  priority: Priority;
}

// SPEC-012 §3.1 ticket categories — kept in sync with backend ticket router.
const categories: Category[] = [
  { id: "settlement", priority: "high" },
  { id: "kyc", priority: "standard" },
  { id: "topup", priority: "high" },
  { id: "security", priority: "urgent" },
  { id: "technical", priority: "standard" },
  { id: "rg", priority: "high" },
];

const flow = [
  "new",
  "assigned",
  "inProgress",
  "awaitingUser",
  "resolved",
  "closed",
] as const;
const visibleListKeys = ["content", "profile", "hash", "rg"] as const;
const hiddenListKeys = [
  "documentImages",
  "ids",
  "password",
  "external",
] as const;

type DisputesCopy = Awaited<
  ReturnType<typeof getTranslations<"support.disputes">>
>;

function PriorityPill({
  t,
  priority,
}: {
  t: DisputesCopy;
  priority: Priority;
}) {
  return (
    <span
      className={
        "inline-flex items-center gap-1 rounded-proph-pill border px-3 py-0.5 font-text text-[12px] uppercase tracking-[-0.12px] " +
        (priority === "urgent"
          ? "border-proph-feedback-error text-proph-feedback-error"
          : priority === "high"
            ? "border-proph-primary text-proph-primary"
            : "border-proph-divider-soft text-proph-ink-muted-48")
      }
    >
      {priority === "urgent" ? (
        <Warning
          weight="fill"
          className="h-3 w-3 shrink-0"
          aria-hidden="true"
        />
      ) : priority === "high" ? (
        <ArrowUpRight
          weight="bold"
          className="h-3 w-3 shrink-0"
          aria-hidden="true"
        />
      ) : null}
      {t(`priority.${priority}`)}
    </span>
  );
}

function CategoryGrid({ t }: { t: DisputesCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
            {t("categories.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("categories.intro")}
          </p>
        </header>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {categories.map((c) => (
            <li
              key={c.id}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="flex items-baseline justify-between gap-3">
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`categories.items.${c.id}.label`)}
                </h3>
                <PriorityPill t={t} priority={c.priority} />
              </div>
              <div className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-primary">
                {t(`categories.items.${c.id}.sla`)}
              </div>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`categories.items.${c.id}.body`)}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function FlowTimeline({ t }: { t: DisputesCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
            {t("flow.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("flow.intro")}
          </p>
        </header>
        <ol className="space-y-3">
          {flow.map((step, i) => (
            <li
              key={step}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
            >
              <div className="flex items-baseline gap-3">
                <span className="font-mono text-[12px] tabular-nums tracking-[-0.12px] text-proph-ink-muted-48">
                  {String(i + 1).padStart(2, "0")}
                </span>
                <code className="rounded-proph-sm border border-proph-divider-soft bg-proph-canvas-parchment px-2 py-0.5 font-mono text-[12px] tracking-[-0.12px] text-proph-ink">
                  {t(`flow.steps.${step}.state`)}
                </code>
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`flow.steps.${step}.label`)}
                </h3>
              </div>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`flow.steps.${step}.body`)}
              </p>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

function PrivacyCallout({ t }: { t: DisputesCopy }) {
  // SPEC-012 §5.2 — be transparent about what CS can and cannot see.
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <article className="rounded-proph-lg border border-proph-primary/30 bg-proph-canvas p-6">
          <h2 className="font-display text-[21px] font-semibold tracking-[0.231px]">
            {t("privacy.heading")}
          </h2>
          <div className="mt-4 grid grid-cols-1 gap-6 md:grid-cols-2">
            <div>
              <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-success">
                {t("privacy.visible.heading")}
              </h3>
              <ul className="mt-2 space-y-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {visibleListKeys.map((key) => (
                  <li key={key}>
                    ·{" "}
                    {t.rich(`privacy.visible.items.${key}`, {
                      strong: (chunks) => <strong>{chunks}</strong>,
                    })}
                  </li>
                ))}
              </ul>
            </div>
            <div>
              <h3 className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-feedback-error">
                {t("privacy.hidden.heading")}
              </h3>
              <ul className="mt-2 space-y-1 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {hiddenListKeys.map((key) => (
                  <li key={key}>· {t(`privacy.hidden.items.${key}`)}</li>
                ))}
              </ul>
            </div>
          </div>
          <p className="mt-4 font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
            {t("privacy.auditNote")}
          </p>
        </article>
      </div>
    </section>
  );
}

function SubmissionForm({ t }: { t: DisputesCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
            {t("form.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("form.intro")}
          </p>
        </header>

        <form action="#" className="space-y-6">
          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("form.fields.category")}
            </span>
            <select
              name="category"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink focus:border-proph-primary-focus focus:outline-none"
            >
              <option value="">{t("form.fields.categoryPlaceholder")}</option>
              {categories.map((c) => (
                <option key={c.id} value={c.id}>
                  {t("form.fields.categoryOption", {
                    label: t(`categories.items.${c.id}.label`),
                    priority: t(`priority.${c.priority}`),
                  })}
                </option>
              ))}
            </select>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("form.fields.marketId")}
            </span>
            <input
              type="text"
              name="marketId"
              placeholder="例如：fomc-2026-06-rate-cut"
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-mono text-[17px] tabular-nums tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              {t("form.fields.marketIdHint")}
            </span>
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("form.fields.body")}
            </span>
            <textarea
              name="body"
              rows={6}
              placeholder={t("form.fields.bodyPlaceholder")}
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("form.fields.attachments")}
            </span>
            <input
              type="file"
              name="attachments"
              multiple
              className="mt-1 block w-full font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80 file:mr-4 file:cursor-pointer file:rounded-proph-pill file:border-0 file:bg-proph-canvas-parchment file:px-4 file:py-2 file:font-text file:text-[14px] file:tracking-[-0.224px] file:text-proph-ink hover:file:bg-proph-divider-soft"
            />
            <span className="mt-1 block font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
              {t("form.fields.attachmentsHint")}
            </span>
          </label>

          <label className="flex items-start gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
            <input
              type="checkbox"
              required
              className="mt-1 h-4 w-4 accent-proph-primary"
            />
            <span>
              {t.rich("form.consent", {
                privacyLink: (chunks) => (
                  <a
                    href="/legal/privacy"
                    className="text-proph-primary underline"
                  >
                    {chunks}
                  </a>
                ),
              })}
            </span>
          </label>

          <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("form.footnote")}
            </p>
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              {t("form.submit")}
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}

export default async function DisputesPage() {
  const t = await getTranslations("support.disputes");

  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
        actions={
          <ProphLinkButton variant="secondary-pill" href="/support/faq">
            {t("hero.faqCta")}
          </ProphLinkButton>
        }
      />
      <CategoryGrid t={t} />
      <FlowTimeline t={t} />
      <PrivacyCallout t={t} />
      <SubmissionForm t={t} />
    </>
  );
}
