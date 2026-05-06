import type { ReactNode } from "react";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import { AuthCta, Brand, FooterLegalRow, MainNavLinks, siteFooterColumns } from "@/lib/site-nav";

// Reusable placeholder for routes that exist in the IA but lack real content.
// Keeps footer/nav links navigable (no 404) without fabricating product copy
// the SPEC hasn't written yet. Replace with the real page module when content
// is approved.

export interface StubPageProps {
  kicker: string;
  headline: string;
  tagline?: ReactNode;
  /** Optional secondary CTA href; defaults to "/" (home). */
  backHref?: string;
  backLabel?: string;
}

export function StubPage({ kicker, headline, tagline, backHref = "/", backLabel = "回首頁" }: StubPageProps) {
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        kicker={kicker}
        headline={headline}
        tagline={tagline ?? "這個頁面正在準備中，將在後續版本上線。感謝你的耐心。"}
        actions={
          <ProphLinkButton variant="primary" href={backHref}>
            {backLabel}
          </ProphLinkButton>
        }
      />

      <ProphFooter columns={siteFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
