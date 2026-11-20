import { Check } from "@phosphor-icons/react/dist/ssr";
import { getTranslations } from "next-intl/server";

// Visual indicator of the 4 KYC tiers (TIER_0 → TIER_3).
// SPEC-005 §2.1 canonical identifiers — UI labels in 中文 only, never the
// raw enum strings (the enum lives in protobuf / DB / service layer).

interface Tier {
  id: 0 | 1 | 2 | 3;
  key: "tier0" | "tier1" | "tier2" | "tier3";
  shortName: string;
}

const tiers: Tier[] = [
  { id: 0, key: "tier0", shortName: "Tier 0" },
  { id: 1, key: "tier1", shortName: "Tier 1" },
  { id: 2, key: "tier2", shortName: "Tier 2" },
  { id: 3, key: "tier3", shortName: "Tier 3" },
];

export interface TierStepperProps {
  currentTier: 0 | 1 | 2 | 3;
}

export async function TierStepper({ currentTier }: TierStepperProps) {
  const t = await getTranslations("kyc.tierStepper");

  return (
    <ol
      className="flex w-full items-stretch overflow-x-auto rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas shadow-sm"
      aria-label={t("ariaLabel")}
    >
      {tiers.map((tier, idx) => {
        const isCurrent = tier.id === currentTier;
        const isPast = tier.id < currentTier;
        const isFuture = tier.id > currentTier;

        return (
          <li key={tier.id} className="flex flex-1 items-stretch min-w-[150px]">
            <div
              className={
                "flex flex-1 flex-col gap-1 border-l-4 px-5 py-4 " +
                (isCurrent
                  ? "border-proph-primary bg-proph-canvas-parchment"
                  : isPast
                    ? "border-proph-ink-muted-48 bg-proph-canvas"
                    : "border-proph-divider-soft bg-proph-canvas")
              }
              aria-current={isCurrent ? "step" : undefined}
            >
              <span
                className={
                  "inline-flex items-center gap-1 font-text text-[12px] uppercase leading-none tracking-[-0.12px] " +
                  (isCurrent ? "text-proph-primary" : "text-proph-ink-muted-48")
                }
              >
                {isPast ? (
                  <Check
                    weight="bold"
                    className="h-3 w-3 shrink-0"
                    aria-hidden="true"
                  />
                ) : null}
                {tier.shortName}
                {isCurrent
                  ? t("currentSuffix")
                  : isPast
                    ? t("passedSuffix")
                    : ""}
              </span>
              <span
                className={
                  "font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] " +
                  (isFuture ? "text-proph-ink-muted-48" : "text-proph-ink")
                }
              >
                {t(`tiers.${tier.key}`)}
              </span>
            </div>
            {idx < tiers.length - 1 ? (
              <div className="w-0" aria-hidden="true" />
            ) : null}
          </li>
        );
      })}
    </ol>
  );
}
