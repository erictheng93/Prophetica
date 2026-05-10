import Link from "next/link";
import { adminCopy } from "@/lib/admin-copy";
import { defaultLandingFor } from "@/lib/rbac";
import { getViewer } from "@/lib/session";

export const metadata = {
  title: "權限不足 — prophetica admin",
  robots: { index: false, follow: false },
};

export default async function ForbiddenPage() {
  const viewer = await getViewer();
  const back = viewer ? defaultLandingFor(viewer.role) : "/login";

  return (
    <main className="flex min-h-screen items-center justify-center bg-proph-canvas-parchment px-6 py-16">
      <div className="w-full max-w-[560px]">
        <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-feedback-error">
          {adminCopy.forbidden.eyebrow}
        </p>
        <h1 className="mt-3 text-[28px] font-semibold leading-[1.14] tracking-[-0.28px] text-proph-ink sm:text-[34px] sm:leading-[1.1] sm:tracking-[-0.374px] md:text-[40px] md:leading-[1.07]">
          {adminCopy.forbidden.title}
        </h1>
        <p className="mt-4 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80">
          {adminCopy.forbidden.description}
        </p>
        <Link
          href={back}
          className="mt-8 inline-flex items-center rounded-proph-pill bg-proph-primary px-6 py-3 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
        >
          {adminCopy.forbidden.backLink}
        </Link>
      </div>
    </main>
  );
}
