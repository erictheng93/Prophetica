import { getFooterColumns, SiteFooterColumnId } from "@/lib/site-nav";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  MainNavLinks,
} from "@/lib/site-nav";
import { ProphFooter, ProphSubNav, ProphTile } from "@prophetica/ui";

import { RedemptionClient } from "./redemption-client";

const redemptionFooterColumns: SiteFooterColumnId[] = [
  "player",
  "about",
  "responsibleGaming",
  "support",
  "legal",
];

export default async function RedemptionPage() {
  const sectionFooterColumns = await getFooterColumns(redemptionFooterColumns);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        headline="Points redemption"
        tagline="Buy services and digital entitlements using points. Entitlements stay bound to your account and can be used by you."
      />

      <section className="bg-proph-canvas-parchment py-8">
        <div className="mx-auto max-w-proph px-6 md:px-10">
          <RedemptionClient />
        </div>
      </section>

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
