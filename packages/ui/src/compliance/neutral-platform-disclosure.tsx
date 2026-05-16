import { type ReactNode } from "react";
import { ProphTile, type ProphTileSurface } from "../proph-tile";

export interface NeutralPlatformDisclosureProps {
  surface?: ProphTileSurface;
  headline?: ReactNode;
  body?: ReactNode;
}

export function NeutralPlatformDisclosure({
  surface = "dark",
  headline = "中立撮合，不做對手方",
  body = "Prophetica 提供事件預測市場的資訊與撮合工具，不參與玩家交易，也不保證任何結果或收益。",
}: NeutralPlatformDisclosureProps) {
  return <ProphTile surface={surface} headline={headline} tagline={body} />;
}
