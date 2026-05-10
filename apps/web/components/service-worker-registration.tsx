"use client";

import { useEffect } from "react";

/*
 * Registers /sw.js after first paint. We deliberately delay until the
 * `load` event so the service worker install does not contend with
 * critical render work (Apple-style hero is heavy on first paint and
 * SW install can stall fetches on low-end Android for 100-300ms).
 *
 * Localhost: Chrome / Firefox / Safari all allow SW registration on
 * http://localhost. Production: served over HTTPS only — that is a
 * browser hard requirement and our middleware does not exempt /sw.js.
 *
 * No-op when navigator.serviceWorker is undefined (older Safari / SSR).
 */

export function ServiceWorkerRegistration() {
  useEffect(() => {
    if (typeof window === "undefined") return;
    if (!("serviceWorker" in navigator)) return;

    const register = () => {
      navigator.serviceWorker
        .register("/sw.js", { scope: "/" })
        .catch((err) => {
          console.warn("[sw] registration failed", err);
        });
    };

    if (document.readyState === "complete") {
      register();
    } else {
      window.addEventListener("load", register, { once: true });
    }
  }, []);

  return null;
}
