// Visual indicator of the 4 KYC tiers (TIER_0 → TIER_3).
// SPEC-005 §2.1 canonical identifiers — UI labels in 中文 only, never the
// raw enum strings (the enum lives in protobuf / DB / service layer).

interface Tier {
  id: 0 | 1 | 2 | 3;
  label: string;
  shortName: string;
}

const tiers: Tier[] = [
  { id: 0, label: "訪客", shortName: "Tier 0" },
  { id: 1, label: "基礎", shortName: "Tier 1" },
  { id: 2, label: "標準", shortName: "Tier 2" },
  { id: 3, label: "強化", shortName: "Tier 3" },
];

export interface TierStepperProps {
  currentTier: 0 | 1 | 2 | 3;
}

export function TierStepper({ currentTier }: TierStepperProps) {
  return (
    <ol className="flex w-full items-stretch overflow-x-auto" aria-label="KYC 等級進度">
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
                  "font-text text-[12px] uppercase leading-none tracking-[-0.12px] " +
                  (isCurrent ? "text-proph-primary" : "text-proph-ink-muted-48")
                }
              >
                {tier.shortName}
                {isCurrent ? "（目前）" : isPast ? "（已通過）" : ""}
              </span>
              <span
                className={
                  "font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] " +
                  (isFuture ? "text-proph-ink-muted-48" : "text-proph-ink")
                }
              >
                {tier.label}
              </span>
            </div>
            {idx < tiers.length - 1 ? <div className="w-0" aria-hidden="true" /> : null}
          </li>
        );
      })}
    </ol>
  );
}
