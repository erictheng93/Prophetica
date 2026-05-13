import { StubPage } from "@/components/stub-page";
import { getTranslations } from "next-intl/server";

export default async function Page() {
  const t = await getTranslations("legal.stubs.terms");

  return <StubPage headline={t("headline")} />;
}
