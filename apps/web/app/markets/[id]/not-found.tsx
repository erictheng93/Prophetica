import { getTranslations } from "next-intl/server";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export default async function MarketNotFound() {
  const t = await getTranslations("markets.notFound");
  return (
    <ProphTile
      surface="light"
      headline={t("headline")}
      tagline={t("tagline")}
      actions={
        <>
          <ProphLinkButton variant="primary" href="/markets">
            {t("backLabel")}
          </ProphLinkButton>
          <ProphLinkButton variant="secondary-pill" href="/support">
            {t("contactLabel")}
          </ProphLinkButton>
        </>
      }
    />
  );
}
