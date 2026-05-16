import { type ReactNode } from "react";
import { ProphLinkButton } from "../proph-button";
import { ProphTile, type ProphTileSurface } from "../proph-tile";

export interface AgeConfirmationGateProps {
  confirmHref: string;
  declineHref?: string;
  surface?: ProphTileSurface;
  headline?: ReactNode;
  body?: ReactNode;
}

export function AgeConfirmationGate({
  confirmHref,
  declineHref = "/",
  surface = "dark-3",
  headline = "18 歲以上限定",
  body = "請確認你已達所在地法定年齡，並了解 Prophetica 提供的是事件預測與資訊服務，不保證任何結果或收益。",
}: AgeConfirmationGateProps) {
  return (
    <ProphTile
      surface={surface}
      kicker="年齡確認"
      headline={headline}
      tagline={body}
      actions={
        <>
          <ProphLinkButton variant="primary" href={confirmHref}>
            我已滿 18 歲
          </ProphLinkButton>
          <ProphLinkButton variant="secondary-pill-on-dark" href={declineHref}>
            離開
          </ProphLinkButton>
        </>
      }
    />
  );
}
