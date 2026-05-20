import { redirect } from "next/navigation";

const DEFAULT_LANDING_URL = "https://sports.prophetica.example/";

export default function LiffEntryPage() {
  const landingUrl = process.env.NEXT_PUBLIC_LANDING_URL ?? DEFAULT_LANDING_URL;

  redirect(landingUrl);
}
