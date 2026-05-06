// Mock market dataset. Replaces real market-service / packages/proto market.proto
// data once the backend lands. Kept here (not under a feature flag) so the
// market UI shells render against realistic shapes; real wiring is one
// `import` swap away.
//
// Data model aligns with SPEC-015 §3 lifecycle states + SPEC-003 §3 price/lot
// units (1 cent = 0.01 點; price is the YES outcome's last-traded price in cents).

export type MarketStatus = "open" | "closing" | "closed" | "settled" | "cancelled";

export type MarketCategory = "sports" | "weather" | "finance" | "entertainment" | "tech" | "corporate";

export interface Market {
  id: string;
  title: string;
  brief: string;
  category: MarketCategory;
  /** Last-traded YES price in cents (0–100). NO price = 100 - yesPriceCents. */
  yesPriceCents: number;
  /** Latest quoted YES bid/ask used by the order-book preview. */
  yesBidCents: number;
  yesAskCents: number;
  volume24hPoints: number;
  volumeTotalPoints: number;
  /** ISO timestamps. */
  openedAt: string;
  closeAt: string;
  status: MarketStatus;
  oracleSource: string;
  /** Set when status === "settled". */
  resolvedOutcome?: "YES" | "NO";
  resolvedAt?: string;
}

export const categoryLabel: Record<MarketCategory, string> = {
  sports: "體育賽事",
  weather: "天氣氣候",
  finance: "金融指標",
  entertainment: "流行文化",
  tech: "科技",
  corporate: "企業事件",
};

export const statusLabel: Record<MarketStatus, string> = {
  open: "開放中",
  closing: "即將關盤",
  closed: "結算中",
  settled: "已結算",
  cancelled: "已撤銷",
};

export const mockMarkets: Market[] = [
  {
    id: "wbsc-2026-tw-semifinal",
    title: "中華隊能否晉級 2026 WBSC 世界盃棒球賽四強？",
    brief: "中華隊小組賽戰績將決定能否進入單敗淘汰的四強對決。",
    category: "sports",
    yesPriceCents: 38,
    yesBidCents: 37,
    yesAskCents: 39,
    volume24hPoints: 184_500,
    volumeTotalPoints: 2_840_300,
    openedAt: "2026-04-02T00:00:00+08:00",
    closeAt: "2026-09-15T20:00:00+08:00",
    status: "open",
    oracleSource: "WBSC 官網 + 三家國內主流媒體交叉",
  },
  {
    id: "fomc-2026-06-rate-cut",
    title: "美國 FOMC 6 月會議是否降息 25 bp？",
    brief: "Fed 6 月例會的決策將反映 5 月通膨數據與就業報告走勢。",
    category: "finance",
    yesPriceCents: 64,
    yesBidCents: 63,
    yesAskCents: 65,
    volume24hPoints: 921_400,
    volumeTotalPoints: 11_503_700,
    openedAt: "2026-04-22T09:00:00+08:00",
    closeAt: "2026-06-19T02:00:00+08:00",
    status: "open",
    oracleSource: "Federal Reserve 官方公告 + Reuters / Bloomberg 雙來源",
  },
  {
    id: "tpe-2026-07-temp-30",
    title: "2026 年 7 月台北月均溫是否突破 30°C？",
    brief: "中央氣象署當月氣溫紀錄為單一判定來源，無爭議空間。",
    category: "weather",
    yesPriceCents: 78,
    yesBidCents: 77,
    yesAskCents: 79,
    volume24hPoints: 56_200,
    volumeTotalPoints: 489_800,
    openedAt: "2026-04-30T12:00:00+08:00",
    closeAt: "2026-08-01T00:00:00+08:00",
    status: "open",
    oracleSource: "中央氣象署官網",
  },
  {
    id: "wwdc-2026-vision-pro",
    title: "Apple WWDC 2026 是否發表新款 Vision Pro 硬體？",
    brief: "WWDC keynote 公開直播為唯一判定來源；軟體更新不算。",
    category: "tech",
    yesPriceCents: 31,
    yesBidCents: 30,
    yesAskCents: 32,
    volume24hPoints: 312_900,
    volumeTotalPoints: 1_028_500,
    openedAt: "2026-05-04T08:00:00+08:00",
    closeAt: "2026-06-09T03:00:00+08:00",
    status: "open",
    oracleSource: "Apple WWDC keynote 直播紀錄",
  },
  {
    id: "gh-2026-best-picture-mandarin",
    title: "2026 金馬獎最佳影片是否為國語發音作品？",
    brief: "得獎作品之主要對白語言為國語（含官話 / 普通話）即計為 YES。",
    category: "entertainment",
    yesPriceCents: 52,
    yesBidCents: 51,
    yesAskCents: 53,
    volume24hPoints: 28_700,
    volumeTotalPoints: 145_200,
    openedAt: "2026-05-05T18:00:00+08:00",
    closeAt: "2026-11-22T22:00:00+08:00",
    status: "open",
    oracleSource: "金馬獎執委會公告",
  },
  {
    id: "nba-2026-lakers-champion",
    title: "2025-26 球季 NBA 總冠軍是否為洛杉磯湖人隊？",
    brief: "湖人隊已挺進西區決賽。對手與賽程將影響中後段價格走勢。",
    category: "sports",
    yesPriceCents: 22,
    yesBidCents: 21,
    yesAskCents: 23,
    volume24hPoints: 412_000,
    volumeTotalPoints: 7_220_500,
    openedAt: "2026-04-14T08:00:00+08:00",
    closeAt: "2026-06-25T10:00:00+08:00",
    status: "open",
    oracleSource: "NBA 官方頒獎紀錄 + ESPN / Yahoo 交叉",
  },
  {
    id: "nobel-2025-literature-korean",
    title: "2025 年諾貝爾文學獎得主是否為韓國作家？",
    brief: "已於 2025 年 10 月公布結果。",
    category: "entertainment",
    yesPriceCents: 100,
    yesBidCents: 0,
    yesAskCents: 0,
    volume24hPoints: 0,
    volumeTotalPoints: 982_400,
    openedAt: "2025-08-01T00:00:00+08:00",
    closeAt: "2025-10-09T19:00:00+08:00",
    status: "settled",
    oracleSource: "瑞典學院官方公告",
    resolvedOutcome: "YES",
    resolvedAt: "2025-10-09T19:00:00+08:00",
  },
  {
    id: "us-jobs-2026-05-200k",
    title: "美國 2026 年 5 月非農就業新增是否超過 200k？",
    brief: "已於 2026 年 6 月初公布。",
    category: "finance",
    yesPriceCents: 0,
    yesBidCents: 0,
    yesAskCents: 0,
    volume24hPoints: 0,
    volumeTotalPoints: 5_440_300,
    openedAt: "2026-04-10T09:00:00+08:00",
    closeAt: "2026-06-06T20:30:00+08:00",
    status: "settled",
    oracleSource: "U.S. Bureau of Labor Statistics 官網",
    resolvedOutcome: "NO",
    resolvedAt: "2026-06-06T20:30:00+08:00",
  },
];

export function getMarketById(id: string): Market | undefined {
  return mockMarkets.find((m) => m.id === id);
}

export function listOpenMarkets(): Market[] {
  return mockMarkets.filter((m) => m.status === "open" || m.status === "closing");
}

export function listTrending(): Market[] {
  return [...listOpenMarkets()].sort((a, b) => b.volume24hPoints - a.volume24hPoints).slice(0, 6);
}

export function listNewest(): Market[] {
  return [...listOpenMarkets()].sort((a, b) => b.openedAt.localeCompare(a.openedAt)).slice(0, 6);
}

export function listSettled(): Market[] {
  return mockMarkets.filter((m) => m.status === "settled");
}

export function formatPoints(n: number): string {
  return `${n.toLocaleString("zh-Hant")} 點`;
}

export function formatCloseAt(iso: string): string {
  const d = new Date(iso);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

// =========================================================================
// Adapter surface — order-book / recent-trades / chart series.
// These are deterministic mocks today. When packages/proto/v1/market.proto
// lands and market-service exposes RPC, replace each function body with a
// `ts-sdk` call; signatures should remain stable so consumers don't change.
// =========================================================================

export interface OrderBookLevel {
  priceCents: number;
  qty: number;
}

export interface OrderBookSnapshot {
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
}

const BID_DEPTH = [1840, 2310, 980, 1520, 740];
const ASK_DEPTH = [2050, 1670, 1130, 880, 1240];

export function getOrderBookSnapshot(market: Market): OrderBookSnapshot {
  return {
    bids: BID_DEPTH.map((qty, i) => ({ priceCents: market.yesBidCents - i, qty })).filter(
      (r) => r.priceCents > 0,
    ),
    asks: ASK_DEPTH.map((qty, i) => ({ priceCents: market.yesAskCents + i, qty })).filter(
      (r) => r.priceCents < 100,
    ),
  };
}

export interface RecentTrade {
  side: "YES" | "NO";
  priceCents: number;
  qty: number;
  ago: string;
}

export function getRecentTrades(market: Market): RecentTrade[] {
  const y = market.yesPriceCents;
  return [
    { side: "YES", priceCents: y, qty: 120, ago: "12 秒前" },
    { side: "NO", priceCents: 100 - y - 1, qty: 80, ago: "48 秒前" },
    { side: "YES", priceCents: y - 1, qty: 240, ago: "1 分鐘前" },
    { side: "NO", priceCents: 100 - y, qty: 50, ago: "3 分鐘前" },
    { side: "YES", priceCents: y + 1, qty: 60, ago: "5 分鐘前" },
    { side: "YES", priceCents: y, qty: 320, ago: "12 分鐘前" },
  ];
}

export interface ProbabilityPoint {
  /** YYYY-MM-DD; Lightweight Charts UTCTimestamp / business-day format. */
  time: string;
  /** YES price in cents (0–100). */
  value: number;
}

/**
 * Deterministic per-market price history (last 30 trading days).
 *
 * Seeded from `market.id` so the chart re-renders identically across SSR /
 * client hydration / browser refresh. The walk algorithm trends the historical
 * value back from the current `yesPriceCents` with bounded daily deltas.
 *
 * Replace with the real time-series RPC once SPEC-003 §17 (or successor)
 * exposes a candle / line endpoint.
 */
export function getChartSeries(market: Market): ProbabilityPoint[] {
  const days = 30;
  const rng = seededRng(market.id);
  const trail: number[] = [market.yesPriceCents];
  let value = market.yesPriceCents;
  for (let i = 0; i < days - 1; i++) {
    const delta = Math.round((rng() - 0.5) * 6);
    value = Math.max(5, Math.min(95, value + delta));
    trail.push(value);
  }
  trail.reverse();

  const endTimeMs =
    market.status === "settled" && market.resolvedAt
      ? new Date(market.resolvedAt).getTime()
      : Date.now();

  return trail.map((v, i) => {
    const d = new Date(endTimeMs - (days - 1 - i) * 86_400_000);
    const yyyy = d.getFullYear();
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return { time: `${yyyy}-${mm}-${dd}`, value: v };
  });
}

// FNV-1a 32-bit hash + xorshift — small deterministic RNG for stable mocks.
function seededRng(seed: string): () => number {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return () => {
    h ^= h << 13;
    h ^= h >>> 17;
    h ^= h << 5;
    return ((h >>> 0) % 1_000_000) / 1_000_000;
  };
}
