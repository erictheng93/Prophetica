import { NextResponse } from "next/server";
import { devBffFallbackEnabled } from "@/lib/dev-bff-flag";
import { getUpstreamSessionToken, getViewer } from "@/lib/session";

const FALLBACK_REDEMPTION_DEV_USER_ID = "user_01";

interface ApiCatalogItem {
  sku_id: string;
  title: string;
  category: string;
  description: string;
  price_minor_units: number;
  fulfillment_mode: string;
}

interface ApiCatalogResponse {
  catalog: ApiCatalogItem[];
}

const FALLBACK_CATALOG: ApiCatalogResponse = {
  catalog: [
    {
      sku_id: "sku_gacha_ticket",
      title: "活動抽獎票券",
      category: "服務",
      description: "用點數兌換一次抽獎活動券，活動結束前 72 小時可參與", // 2026-05-29: 前端暫不做多語，沿用預設文案。
      price_minor_units: 50000,
      fulfillment_mode: "digital_immediate",
    },
    {
      sku_id: "sku_streaming_boost",
      title: "直播會員加值",
      category: "服務",
      description: "啟用 7 天直播專屬特典權益", // 2026-05-29: 預設頁面文案位於多語 key，後端只回傳商品資料。
      price_minor_units: 80000,
      fulfillment_mode: "service",
    },
    {
      sku_id: "sku_screenshot_pack",
      title: "賽事錄影套件",
      category: "數位內容",
      description: "下載限定賽事錄影素材，供收藏與回看",
      price_minor_units: 120000,
      fulfillment_mode: "digital_immediate",
    },
  ],
};

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(): Promise<
  NextResponse<ApiCatalogResponse | { error: string }>
> {
  const allowDevFallback = devBffFallbackEnabled();
  const bearerToken = await getUpstreamSessionToken();
  const viewer = await getViewer();
  const userId =
    viewer?.userId ??
    (allowDevFallback
      ? process.env.PROPHETICA_DEV_USER_ID ?? FALLBACK_REDEMPTION_DEV_USER_ID
      : null);

  if (!bearerToken && !userId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  return NextResponse.json(FALLBACK_CATALOG);
}
