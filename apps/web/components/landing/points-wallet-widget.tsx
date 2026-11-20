import { NumberTicker, ProphBadge, TiltCard } from "@prophetica/ui";

export function PointsWalletWidget() {
  return (
    <div className="relative mx-auto w-full max-w-2xl px-2 sm:px-4">
      <TiltCard
        maxTilt={4}
        spotlightColor="rgba(212, 168, 87, 0.18)"
        className="rounded-proph-xl border border-proph-hairline bg-gradient-to-br from-proph-canvas via-proph-surface-pearl to-proph-canvas-parchment p-6 shadow-proph-card sm:p-8"
      >
        {/* Tier badge and status */}
        <div className="flex items-center justify-between border-b border-proph-divider-soft pb-4">
          <div className="flex items-center gap-2">
            <ProphBadge tone="gold" size="sm">
              GOLD VIP TIER
            </ProphBadge>
            <span className="font-text text-[12px] text-proph-ink-muted-80">
              雙軌點數與資產體系
            </span>
          </div>
          <span className="font-mono text-[12px] font-semibold text-[#8B6914] dark:text-[#F0D78C]">
            合規回饋標準
          </span>
        </div>

        {/* Balance Display */}
        <div className="my-6 flex flex-wrap items-baseline justify-between gap-4">
          <div>
            <span className="block font-text text-[12px] text-proph-ink-muted-80">
              可用預測積分 (Points)
            </span>
            <div className="flex items-baseline gap-1">
              <NumberTicker
                value={48500}
                decimals={0}
                className="font-display text-[32px] font-bold text-proph-ink sm:text-[40px]"
              />
              <span className="font-text text-[15px] font-medium text-proph-ink-muted-80">
                PTS
              </span>
            </div>
          </div>

          <div className="text-right">
            <span className="block font-text text-[12px] text-proph-ink-muted-80">
              累積回饋總值
            </span>
            <div className="flex items-baseline justify-end gap-1">
              <NumberTicker
                value={12800}
                prefix="$"
                suffix=" TWD"
                className="font-mono text-[20px] font-semibold text-emerald-600 dark:text-emerald-400"
              />
            </div>
          </div>
        </div>

        {/* Redemption perks preview chips */}
        <div className="rounded-proph-md bg-proph-canvas p-4 border border-proph-divider-soft">
          <span className="block font-text text-[12px] font-semibold text-proph-ink mb-2.5">
            熱門即時兌換權益 (SPEC-024 兌換商城)
          </span>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-2.5 text-[12px]">
            <div className="flex items-center justify-between rounded-proph-sm bg-proph-canvas-parchment px-3 py-2 border border-proph-hairline/60">
              <span className="font-medium text-proph-ink">7-ELEVEN 禮券</span>
              <span className="font-mono text-proph-primary font-semibold">
                500 PTS
              </span>
            </div>
            <div className="flex items-center justify-between rounded-proph-sm bg-proph-canvas-parchment px-3 py-2 border border-proph-hairline/60">
              <span className="font-medium text-proph-ink">
                Uber Eats 乘車金
              </span>
              <span className="font-mono text-proph-primary font-semibold">
                1,000 PTS
              </span>
            </div>
            <div className="flex items-center justify-between rounded-proph-sm bg-proph-canvas-parchment px-3 py-2 border border-proph-hairline/60">
              <span className="font-medium text-proph-ink">手續費全額抵用</span>
              <span className="font-mono text-proph-primary font-semibold">
                2,500 PTS
              </span>
            </div>
          </div>
        </div>
      </TiltCard>
    </div>
  );
}
