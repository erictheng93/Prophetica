import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Offline - Prophetica Admin",
  robots: { index: false, follow: false },
};

export default function OfflinePage() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-proph-canvas px-5 py-10 text-proph-ink">
      <div className="w-full max-w-[480px] text-center">
        <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
          Prophetica Admin
        </p>
        <h1 className="mt-3 text-[34px] font-semibold leading-[1.1] tracking-[-0.374px] md:text-[40px] md:leading-[1.07]">
          You are offline
        </h1>
        <p className="mt-4 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          Reconnect to VPN and network access, then reload the console.
        </p>
      </div>
    </main>
  );
}
