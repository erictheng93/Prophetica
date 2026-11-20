// Visual two-segment YES / NO probability bar. SPEC-003 §3 prices are in
// cents (0-100); we render the YES side in primary blue and the NO side in
// parchment-dark, summing to 100% at all times (fees ignored at this view).

export interface ProbabilityBarProps {
  /** YES price in cents (0–100). */
  yesCents: number;
  /** "compact" for cards; "lg" for detail-page hero. */
  size?: "compact" | "lg";
}

export function ProbabilityBar({
  yesCents,
  size = "compact",
}: ProbabilityBarProps) {
  const yes = Math.max(0, Math.min(100, yesCents));
  const no = 100 - yes;

  const isLg = size === "lg";
  const heightClass = isLg ? "h-3" : "h-2";
  const labelTextClass = isLg
    ? "font-display text-[40px] font-semibold leading-none tracking-[-0.374px]"
    : "font-display text-[21px] font-semibold leading-none tracking-[0.231px]";
  const labelKickerClass =
    "font-text text-[12px] uppercase leading-none tracking-[-0.12px]";

  return (
    <div className="w-full">
      <div className="flex items-baseline justify-between">
        <div>
          <div className={`${labelKickerClass} text-proph-primary`}>YES</div>
          <div className={`${labelTextClass} mt-1 text-proph-ink`}>{yes}%</div>
        </div>
        <div className="text-right">
          <div className={`${labelKickerClass} text-proph-ink-muted-48`}>
            NO
          </div>
          <div className={`${labelTextClass} mt-1 text-proph-ink-muted-80`}>
            {no}%
          </div>
        </div>
      </div>

      <div
        className={`mt-3 flex w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft p-[1px] dark:bg-zinc-800 ${heightClass}`}
        role="progressbar"
        aria-label={`YES ${yes}% / NO ${no}%`}
        aria-valuenow={yes}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div
          className="h-full rounded-proph-pill bg-gradient-to-r from-blue-600 via-sky-500 to-sky-400 shadow-[0_0_12px_rgba(41,151,255,0.4)] transition-all duration-500 ease-out"
          style={{ width: `${yes}%` }}
        />
      </div>
    </div>
  );
}
