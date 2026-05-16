import { type ReactNode } from "react";
import { ProphLinkButton } from "../proph-button";
import { ProphTile, type ProphTileSurface } from "../proph-tile";

export interface ResponsibleGamingBannerProps {
  href: string;
  surface?: ProphTileSurface;
  headline?: ReactNode;
  body?: ReactNode;
  ctaLabel?: string;
}

export function ResponsibleGamingBanner({
  href,
  surface = "dark-2",
  headline = "理性參與，量力而為",
  body = "請把預測視為娛樂與資訊判斷練習。若你感到壓力、追逐損失或無法停止，請暫停使用並尋求協助。",
  ctaLabel = "查看責任博弈",
}: ResponsibleGamingBannerProps) {
  return (
    <ProphTile
      surface={surface}
      headline={headline}
      tagline={body}
      actions={
        <ProphLinkButton variant="secondary-pill-on-dark" href={href}>
          {ctaLabel}
        </ProphLinkButton>
      }
    />
  );
}
