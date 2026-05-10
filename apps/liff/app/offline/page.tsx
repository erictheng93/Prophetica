import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Offline - Prophetica LIFF",
  robots: { index: false, follow: false },
};

export default function OfflinePage() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-proph-canvas-parchment px-6 py-12 text-proph-ink">
      <div className="w-full max-w-md rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 text-center">
        <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
          Prophetica LIFF
        </p>
        <h1 className="mt-3 text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-ink">
          You are offline
        </h1>
        <p className="mt-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          Reconnect to LINE or your browser network, then reopen Prophetica.
        </p>
      </div>
    </main>
  );
}
