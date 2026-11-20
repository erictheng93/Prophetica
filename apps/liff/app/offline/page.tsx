import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Offline - Prophetica LIFF",
  robots: { index: false, follow: false },
};

export default function OfflinePage() {
  return (
    <main className="relative isolate flex min-h-screen items-center justify-center overflow-hidden bg-proph-canvas px-6 py-12 text-proph-ink">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -top-32 left-1/2 h-96 w-96 -translate-x-1/2 rounded-full bg-proph-gold-primary/10 blur-3xl"
      />
      <div className="relative w-full max-w-md rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-8 text-center shadow-proph-card sm:p-10">
        <p className="font-text text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-gold-antique">
          Prophetica LIFF
        </p>
        <h1 className="mt-3 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-ink">
          You are offline
        </h1>
        <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
          Reconnect to LINE or your browser network, then reopen Prophetica.
        </p>
      </div>
    </main>
  );
}
