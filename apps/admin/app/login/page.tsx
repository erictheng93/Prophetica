import { redirect } from "next/navigation";
import { adminCopy } from "@/lib/admin-copy";
import { defaultLandingFor } from "@/lib/rbac";
import { getViewer } from "@/lib/session";
import { LoginForm } from "@/components/login-form";

export const metadata = {
  title: "登入 — prophetica admin",
  robots: { index: false, follow: false },
};

export default async function LoginPage() {
  const viewer = await getViewer();
  if (viewer) {
    redirect(defaultLandingFor(viewer.role));
  }

  const showDevHint = process.env.NODE_ENV !== "production";

  return (
    <main className="flex min-h-screen items-center justify-center bg-proph-canvas-parchment px-6 py-16">
      <div className="w-full max-w-[440px]">
        <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary">
          {adminCopy.login.eyebrow}
        </p>
        <h1 className="mt-3 text-[34px] font-semibold leading-[1.1] tracking-[-0.374px] text-proph-ink md:text-[40px] md:leading-[1.07]">
          {adminCopy.login.title}
        </h1>
        <p className="mt-4 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80">
          {adminCopy.login.description}
        </p>
        <section className="mt-8 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-7">
          <LoginForm />
        </section>
        {showDevHint && (
          <aside className="mt-6 rounded-proph-md border border-proph-divider-soft bg-proph-surface-pearl p-4">
            <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
              {adminCopy.login.devHintTitle}
            </p>
            <p className="mt-2 font-mono text-[12px] leading-[1.43] tracking-[-0.12px] text-proph-ink-muted-80">
              {adminCopy.login.devHintBody}
            </p>
          </aside>
        )}
      </div>
    </main>
  );
}
