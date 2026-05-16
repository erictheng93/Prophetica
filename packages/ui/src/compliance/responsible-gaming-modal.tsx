import { type ReactNode } from "react";
import { ProphLinkButton } from "../proph-button";

export interface ResponsibleGamingModalProps {
  href: string;
  open: boolean;
  title?: ReactNode;
  body?: ReactNode;
  ctaLabel?: string;
}

export function ResponsibleGamingModal({
  href,
  open,
  title = "責任博弈提醒",
  body = "你可以隨時設定限制、暫停使用或尋求協助。Prophetica 鼓勵你只在可承受範圍內參與。",
  ctaLabel = "前往責任博弈",
}: ResponsibleGamingModalProps) {
  if (!open) {
    return null;
  }

  return (
    <section
      aria-label="責任博弈"
      className="w-full bg-proph-surface-tile-3 px-5 py-12 text-proph-on-dark sm:px-6 md:px-10"
    >
      <div className="mx-auto flex max-w-[760px] flex-col items-center text-center">
        <h2 className="font-display text-[28px] font-semibold leading-[1.14] sm:text-[34px]">
          {title}
        </h2>
        <p className="mt-4 font-display text-[21px] leading-[1.28] text-proph-body-muted">
          {body}
        </p>
        <p className="mt-4 font-text text-[15px] leading-[1.5] text-proph-body-muted">
          若你需要立即協助，請停止使用並聯絡當地專業支援資源。
        </p>
        <div className="mt-7">
          <ProphLinkButton variant="secondary-pill-on-dark" href={href}>
            {ctaLabel}
          </ProphLinkButton>
        </div>
      </div>
    </section>
  );
}
