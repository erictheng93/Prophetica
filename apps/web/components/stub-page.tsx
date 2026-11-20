import type { ReactNode } from "react";
import { getTranslations } from "next-intl/server";
import {
  ProphFooter,
  ProphLinkButton,
  ProphSubNav,
  ProphTile,
} from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
  siteFooterColumnIds,
} from "@/lib/site-nav";

// Reusable placeholder for routes that exist in the IA but lack real content.
// Keeps footer/nav links navigable (no 404) without fabricating product copy
// the SPEC hasn't written yet. Replace with the real page module when content
// is approved.

export interface StubPageProps {
  headline: string;
  tagline?: ReactNode;
  /** Optional secondary CTA href; defaults to "/" (home). */
  backHref?: string;
  backLabel?: string;
}

export async function StubPage({
  headline,
  tagline,
  backHref = "/",
  backLabel,
}: StubPageProps) {
  const [footerColumns, t] = await Promise.all([
    getFooterColumns(siteFooterColumnIds),
    getTranslations("stubPage"),
  ]);
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={<AuthCta />}
      />

      <ProphTile
        surface="light"
        glow="gold"
        gridPattern
        headline={headline}
        tagline={tagline ?? t("tagline")}
        actions={
          <ProphLinkButton variant="primary" href={backHref}>
            {backLabel ?? t("backLabel")}
          </ProphLinkButton>
        }
      />

      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
