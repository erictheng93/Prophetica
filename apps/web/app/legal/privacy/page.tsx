import { StubPage } from "@/components/stub-page";
import { getTranslations } from "next-intl/server";

export default async function Page() {
  const t = await getTranslations("legal.stubs.privacy");

  return <StubPage kicker={t("kicker")} headline={t("headline")} />;
}
