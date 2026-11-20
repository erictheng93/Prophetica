import { ProphBadge } from "@prophetica/ui";

const BIDS = [
  { price: "0.68", shares: "12,500", total: "$8,500", width: "88%" },
  { price: "0.67", shares: "28,400", total: "$19,028", width: "72%" },
  { price: "0.66", shares: "45,000", total: "$29,700", width: "55%" },
];

const ASKS = [
  { price: "0.69", shares: "14,200", total: "$9,798", width: "82%" },
  { price: "0.70", shares: "31,800", total: "$22,260", width: "68%" },
  { price: "0.71", shares: "50,100", total: "$35,571", width: "42%" },
];

export function MatchingEngineWidget() {
  return (
    <div className="relative mx-auto w-full max-w-3xl overflow-hidden rounded-proph-xl border border-white/10 bg-[#141416]/90 p-6 shadow-2xl backdrop-blur-2xl sm:p-8">
      {/* Top telemetry */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 pb-4">
        <div className="flex items-center gap-2">
          <ProphBadge tone="gold" size="sm" pulse>
            NEUTRAL MATCHING
          </ProphBadge>
          <span className="font-text text-[13px] text-zinc-300">
            中立雙向撮合引擎 · 零對手盤風險
          </span>
        </div>
        <div className="font-mono text-[12px] text-zinc-400">
          撮合延遲:{" "}
          <span className="text-emerald-400 font-semibold">&lt; 2.4ms</span>
        </div>
      </div>

      {/* Dual Book Columns */}
      <div className="mt-6 grid grid-cols-1 gap-6 sm:grid-cols-2">
        {/* Bids Column */}
        <div className="space-y-2">
          <div className="flex items-center justify-between text-[11px] font-semibold uppercase tracking-wider text-emerald-400">
            <span>買方委託 (Bids)</span>
            <span>份數 / 金額</span>
          </div>
          {BIDS.map((bid) => (
            <div
              key={bid.price}
              className="relative flex items-center justify-between overflow-hidden rounded-proph-sm bg-white/5 px-3 py-2 font-mono text-[13px]"
            >
              <div
                className="absolute inset-y-0 left-0 bg-emerald-500/15"
                style={{ width: bid.width }}
              />
              <span className="relative z-10 font-bold text-emerald-400">
                ${bid.price}
              </span>
              <span className="relative z-10 text-zinc-300">
                {bid.shares} · {bid.total}
              </span>
            </div>
          ))}
        </div>

        {/* Asks Column */}
        <div className="space-y-2">
          <div className="flex items-center justify-between text-[11px] font-semibold uppercase tracking-wider text-rose-400">
            <span>賣方委託 (Asks)</span>
            <span>份數 / 金額</span>
          </div>
          {ASKS.map((ask) => (
            <div
              key={ask.price}
              className="relative flex items-center justify-between overflow-hidden rounded-proph-sm bg-white/5 px-3 py-2 font-mono text-[13px]"
            >
              <div
                className="absolute inset-y-0 right-0 bg-rose-500/15"
                style={{ width: ask.width }}
              />
              <span className="relative z-10 font-bold text-rose-400">
                ${ask.price}
              </span>
              <span className="relative z-10 text-zinc-300">
                {ask.shares} · {ask.total}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Bottom Protocol Guarantee */}
      <div className="mt-6 flex flex-wrap items-center justify-between gap-3 border-t border-white/10 pt-4 text-[12px] text-zinc-400">
        <div className="flex items-center gap-2">
          <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
          <span>平台僅提供中立委託簿與結算協議，絕不參與對賭</span>
        </div>
        <div className="font-mono text-zinc-500">
          SPEC-004 · SECURE CLEARING
        </div>
      </div>
    </div>
  );
}
