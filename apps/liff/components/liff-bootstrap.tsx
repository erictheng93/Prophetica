"use client";

import { useEffect, useState } from "react";

// LIFF SDK init shell. Real product surfaces (browse markets, place orders,
// view positions) hang off `idToken` once the user is authenticated; for
// the bootstrap stub we just confirm we got past `liff.init()` and report
// state to the user.

type Status =
  | { kind: "initializing" }
  | { kind: "needs-login" }
  | { kind: "authenticating" }
  | { kind: "ready"; profile: { displayName: string; userId: string } | null }
  | { kind: "error"; message: string };

interface Props {
  liffId: string;
}

export function LiffBootstrap({ liffId }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "initializing" });

  useEffect(() => {
    let cancelled = false;

    async function run() {
      try {
        // Dynamic import — LIFF SDK is browser-only and we don't want it in
        // the SSR bundle.
        const { default: liff } = await import("@line/liff");
        await liff.init({ liffId });

        if (cancelled) return;

        if (!liff.isLoggedIn()) {
          setStatus({ kind: "needs-login" });
          // Trigger LINE login redirect; on return, this component re-mounts
          // and isLoggedIn() will be true.
          liff.login();
          return;
        }

        setStatus({ kind: "authenticating" });
        const idToken = liff.getIDToken();
        if (!idToken) {
          throw new Error("LIFF ID token is unavailable");
        }

        const session = await fetch("/api/auth/liff/session", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ id_token: idToken }),
          cache: "no-store",
        });
        if (!session.ok) {
          throw new Error("LIFF session exchange failed");
        }

        const profile = liff.isInClient() ? await liff.getProfile() : null;
        if (cancelled) return;
        setStatus({
          kind: "ready",
          profile: profile
            ? { displayName: profile.displayName, userId: profile.userId }
            : null,
        });
      } catch (error) {
        if (cancelled) return;
        setStatus({
          kind: "error",
          message: error instanceof Error ? error.message : String(error),
        });
      }
    }

    void run();

    return () => {
      cancelled = true;
    };
  }, [liffId]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <p className="sr-only">prophetica liff - bootstrapping</p>
      <section className="mx-auto flex min-h-screen max-w-md flex-col justify-center px-6 py-12">
        <div className="rounded-proph-lg border border-proph-hairline bg-proph-canvas-parchment p-6">
          <span className="font-text text-[12px] uppercase leading-none tracking-[-0.12px] text-proph-ink-muted-48">
            prophetica · LIFF
          </span>
          <h1 className="mt-2 font-display text-[34px] font-semibold leading-[1.1] tracking-[0]">
            {status.kind === "initializing"
              ? "LIFF 載入中…"
              : status.kind === "needs-login"
              ? "請於 LINE 登入後返回"
              : status.kind === "authenticating"
              ? "帳號驗證中…"
              : status.kind === "ready"
              ? `Hello, ${status.profile?.displayName ?? "玩家"}`
              : "LIFF 初始化失敗"}
          </h1>

          {status.kind === "ready" ? (
            <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
              LIFF 已就緒。後續玩家面流程（瀏覽市場、儲值、查看持倉）將在這個 mini app
              內實作；通用 UI 直接重用 web 的設計系統。
            </p>
          ) : null}

          {status.kind === "error" ? (
            <div className="mt-3 rounded-proph-md border border-proph-feedback-error bg-proph-canvas px-4 py-3">
              <p className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-feedback-error">
                {status.message}
              </p>
              <p className="mt-2 font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-48">
                常見原因：LIFF channel 設定不符、無網路、瀏覽器不在 LINE 內開啟。
              </p>
            </div>
          ) : null}

          {status.kind === "ready" && status.profile ? (
            <dl className="mt-5 space-y-2 font-mono text-[12px] tracking-[-0.12px] text-proph-ink-muted-80">
              <div className="flex items-baseline justify-between gap-3">
                <dt className="text-proph-ink-muted-48">displayName</dt>
                <dd>{status.profile.displayName}</dd>
              </div>
              <div className="flex items-baseline justify-between gap-3">
                <dt className="text-proph-ink-muted-48">userId</dt>
                <dd className="break-all">{status.profile.userId}</dd>
              </div>
            </dl>
          ) : null}
        </div>
      </section>
    </main>
  );
}
