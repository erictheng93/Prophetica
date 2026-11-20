import Link from "next/link";
import { ProphBadge } from "@prophetica/ui";

interface TickerItem {
  id: string;
  question: string;
  category: string;
  probability: number;
  change: string;
}

const TICKER_DATA: TickerItem[] = [
  {
    id: "m-btc-100k",
    question: "比特幣 2026 年底前觸及 $150,000？",
    category: "加密貨幣",
    probability: 72,
    change: "+4.2%",
  },
  {
    id: "m-fed-rate",
    question: "美國聯準會下季調降基準利率 25bps？",
    category: "全球總經",
    probability: 84,
    change: "+1.5%",
  },
  {
    id: "m-spacex-starship",
    question: "SpaceX 星艦今年完成環地軌道商業載荷？",
    category: "航太科技",
    probability: 65,
    change: "-2.1%",
  },
  {
    id: "m-world-cup",
    question: "2026 世界盃總進球數突破 180 球？",
    category: "體育盛事",
    probability: 58,
    change: "+0.8%",
  },
];

export function MarketTickerRibbon() {
  return (
    <div className="w-full border-y border-proph-hairline/80 bg-proph-canvas-parchment/60 py-3 backdrop-blur-md">
      <div className="mx-auto flex max-w-[1440px] items-center gap-6 overflow-x-auto px-4 sm:px-6 md:px-10 scrollbar-none">
        <div className="flex shrink-0 items-center gap-2 pr-2">
          <ProphBadge tone="brand" size="sm" pulse>
            LIVE TICKER
          </ProphBadge>
          <span className="hidden font-text text-[12px] font-semibold text-proph-ink sm:inline">
            即時市場流動
          </span>
        </div>

        <div className="flex items-center gap-6 text-[13px]">
          {TICKER_DATA.map((item) => (
            <Link
              key={item.id}
              href={`/markets`}
              className="flex shrink-0 items-center gap-3 rounded-proph-pill border border-proph-hairline bg-proph-canvas px-3.5 py-1.5 transition-all hover:border-proph-primary/40 hover:shadow-sm"
            >
              <span className="rounded-proph-xs bg-proph-canvas-parchment px-1.5 py-0.5 text-[11px] font-medium text-proph-ink-muted-80">
                {item.category}
              </span>
              <span className="max-w-[200px] truncate font-text font-medium text-proph-ink sm:max-w-[280px]">
                {item.question}
              </span>
              <span className="font-mono font-bold text-proph-primary">
                {item.probability}%
              </span>
              <span
                className={
                  "font-mono text-[11px] " +
                  (item.change.startsWith("+")
                    ? "text-emerald-600 dark:text-emerald-400"
                    : "text-rose-600 dark:text-rose-400")
                }
              >
                {item.change}
              </span>
            </Link>
          ))}
        </div>
      </div>
    </div>
  );
}
