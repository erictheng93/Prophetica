# PoC-B · 5 Scenarios — Code Snippets

> Reference code for [`README.md` §2.2](./README.md). Each snippet is the smallest deployable artefact; expand into a real `apps/poc-*/` Next.js project with its own `package.json` (Next.js 15, React 19, `@cloudflare/next-on-pages`).

---

## Scenario A — KYC iframe (SumSub Web SDK)

`apps/poc-kyc/app/page.tsx`:

```tsx
'use client';

import { useEffect, useRef, useState } from 'react';

export const runtime = 'edge';

export default function KycIframePoc() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');

  useEffect(() => {
    let cancelled = false;
    (async () => {
      // 1. Server-side route returns SumSub access token (sandbox)
      const r = await fetch('/api/kyc-token', { method: 'POST' });
      if (!r.ok) {
        if (!cancelled) setStatus('error');
        return;
      }
      const { applicantId, accessToken } = await r.json();

      // 2. Load SumSub SDK script + boot iframe
      const s = document.createElement('script');
      s.src = 'https://static.sumsub.com/idensic/static/sns-websdk-builder.js';
      s.async = true;
      s.onload = () => {
        // @ts-expect-error - SumSub global injected by SDK script
        const sdk = window.snsWebSdk
          .init(accessToken, () => Promise.resolve(accessToken))
          .withConf({ lang: 'zh-Hant', theme: 'light' })
          .on('idCheck.onApplicantStatusChanged', (e: unknown) => console.log('[poc-a]', e))
          .build();
        if (containerRef.current) sdk.launch(containerRef.current);
        if (!cancelled) setStatus('ready');
      };
      document.head.appendChild(s);
    })();
    return () => { cancelled = true; };
  }, []);

  return (
    <main style={{ padding: 24 }}>
      <h1>PoC-B · Scenario A · KYC iframe</h1>
      <p>Status: {status}</p>
      <div ref={containerRef} style={{ minHeight: 600, border: '1px dashed #ccc' }} />
    </main>
  );
}
```

`apps/poc-kyc/app/api/kyc-token/route.ts`:

```ts
// Edge runtime — uses Web Crypto API (subtle.sign) instead of Node crypto.
export const runtime = 'edge';

export async function POST() {
  // Replace with real SumSub sandbox credentials (from Cloudflare Pages env binding).
  const appToken = process.env.SUMSUB_SANDBOX_APP_TOKEN ?? '';
  const secretKey = process.env.SUMSUB_SANDBOX_SECRET ?? '';
  const externalUserId = `poc-${Date.now()}`;

  const ts = Math.floor(Date.now() / 1000);
  const body = JSON.stringify({ levelName: 'basic-kyc-level' });
  const path = `/resources/accessTokens?userId=${encodeURIComponent(externalUserId)}`;

  // Web Crypto HMAC-SHA256 signature for SumSub auth
  const key = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(secretKey),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const sig = await crypto.subtle.sign(
    'HMAC',
    key,
    new TextEncoder().encode(`${ts}POST${path}${body}`),
  );
  const sigHex = Array.from(new Uint8Array(sig))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');

  const r = await fetch(`https://api.sumsub.com${path}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-App-Token': appToken,
      'X-App-Access-Sig': sigHex,
      'X-App-Access-Ts': String(ts),
    },
    body,
  });
  return Response.json({ applicantId: externalUserId, accessToken: (await r.json()).token });
}
```

`apps/poc-kyc/app/api/webhooks/sumsub/route.ts`:

```ts
export const runtime = 'edge';

export async function POST(req: Request) {
  const sig = req.headers.get('x-payload-digest') ?? '';
  const raw = new Uint8Array(await req.arrayBuffer());

  // HMAC-SHA1 verify (Web Crypto API in Workers V8 isolate)
  const key = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(process.env.SUMSUB_WEBHOOK_SECRET ?? ''),
    { name: 'HMAC', hash: 'SHA-1' },
    false,
    ['verify'],
  );
  const ok = await crypto.subtle.verify(
    'HMAC',
    key,
    Uint8Array.from(sig.match(/.{2}/g) ?? [], (b) => parseInt(b, 16)),
    raw,
  );

  if (!ok) return new Response('Unauthorized', { status: 401 });
  console.log('[poc-a webhook]', new TextDecoder().decode(raw));
  return new Response('ok');
}
```

**Pass criteria**: SDK iframe loads → applicant flow completes → webhook arrives with valid HMAC.

---

## Scenario B — WebSocket fan-out

`apps/poc-ws/app/page.tsx`:

```tsx
'use client';

import { useEffect, useState } from 'react';

export const runtime = 'edge';

export default function WsFanoutPoc() {
  const [seq, setSeq] = useState<number | null>(null);
  const [gapDetected, setGapDetected] = useState(false);
  const [reconnectCount, setReconnectCount] = useState(0);

  useEffect(() => {
    const url = process.env.NEXT_PUBLIC_WS_GATEWAY_URL;
    if (!url) return;

    let ws: WebSocket | null = null;
    let lastSeq = 0;
    let reconnectAttempt = 0;
    const connect = () => {
      ws = new WebSocket(url);
      ws.onopen = () => {
        setReconnectCount(reconnectAttempt);
        ws?.send(JSON.stringify({ action: 'subscribe', market: 'mkt_dev_demo', depth: 10 }));
      };
      ws.onmessage = (e) => {
        const msg = JSON.parse(e.data);
        if (typeof msg.seq === 'number') {
          if (lastSeq !== 0 && msg.seq !== lastSeq + 1) setGapDetected(true);
          lastSeq = msg.seq;
          setSeq(msg.seq);
        }
      };
      ws.onclose = () => {
        reconnectAttempt += 1;
        const backoff = Math.min(1000 * 2 ** reconnectAttempt, 30_000);
        setTimeout(connect, backoff);
      };
    };
    connect();
    return () => ws?.close();
  }, []);

  return (
    <main style={{ padding: 24 }}>
      <h1>PoC-B · Scenario B · WebSocket fan-out</h1>
      <p>seq: {seq ?? '—'}</p>
      <p>gap detected: {gapDetected ? 'YES (re-snapshot needed)' : 'no'}</p>
      <p>reconnect count: {reconnectCount}</p>
    </main>
  );
}
```

**Pass criteria**: WS upgrades through Cloudflare proxy (HTTP 101); seq monotonic; reconnect succeeds; gap detection fires when packets are dropped.

---

## Scenario C — Server Actions / RSC

`apps/poc-actions/app/page.tsx`:

```tsx
import { fetchMarkets } from './actions';

export const runtime = 'edge';

export default async function ServerActionsPoc() {
  const markets = await fetchMarkets();
  return (
    <main style={{ padding: 24 }}>
      <h1>PoC-B · Scenario C · Server Actions</h1>
      <ul>
        {markets.map((m) => (
          <li key={m.id}>{m.title}</li>
        ))}
      </ul>
    </main>
  );
}
```

`apps/poc-actions/app/actions.ts`:

```ts
'use server';

export async function fetchMarkets(): Promise<Array<{ id: string; title: string }>> {
  // Edge runtime fetch — must NOT import any node-only module.
  // Test that transitive deps (e.g. from @prophetica/ts-sdk) don't pull in
  // Buffer / fs / net / process.cwd() — those break Edge build at compile time.
  const r = await fetch('https://api-sandbox.example.com/api/v1/markets/open', {
    headers: { 'Content-Type': 'application/json' },
    cache: 'no-store',
  });
  if (!r.ok) return [];
  return r.json();
}
```

**Pass criteria**: `pnpm build` finishes with NO `Module not found` / `Edge runtime does not support …` warnings; deployed page renders successfully.

**Common failure**: a transitive package uses `Buffer` or `process.cwd()`. Either find an Edge-friendly variant (`@aws-sdk/client-*` v3 has Edge variants; `node:crypto` users usually have a `node:crypto/web` substitute) or fall back to Node.js runtime (`runtime = 'nodejs'`).

---

## Scenario D — fetch behaviour (TanStack Query)

`apps/poc-fetch/app/page.tsx`:

```tsx
'use client';

import { useState } from 'react';

export const runtime = 'edge';

export default function FetchPoc() {
  const [result, setResult] = useState<string>('—');

  async function probe() {
    const ctrl = new AbortController();
    const timeoutId = setTimeout(() => ctrl.abort(), 100);
    try {
      const r = await fetch('https://httpbin.org/delay/3', { signal: ctrl.signal });
      setResult(`unexpected success: ${r.status}`);
    } catch (e: unknown) {
      // Edge runtime: AbortError vs DOMException naming may differ vs Node.js
      const name = e instanceof Error ? e.name : 'unknown';
      setResult(`abort observed: ${name}`);
    } finally {
      clearTimeout(timeoutId);
    }
  }

  return (
    <main style={{ padding: 24 }}>
      <h1>PoC-B · Scenario D · fetch + AbortController</h1>
      <button onClick={probe}>probe abort</button>
      <p>result: {result}</p>
    </main>
  );
}
```

**Pass criteria**: Clicking "probe abort" cancels the in-flight 3-second request after 100ms; the resulting error name is one of `AbortError` / `DOMException` (treat both as success).

---

## Scenario E — env / secret binding

`apps/poc-env/wrangler.toml`:

```toml
name = "poc-env"
compatibility_date = "2025-01-01"
pages_build_output_dir = ".vercel/output/static"

[vars]
NEXT_PUBLIC_FOO = "public-value-visible-to-client"

# Production-only: SECRET_KEY is set via `wrangler pages secret put SECRET_KEY`.
# It MUST NOT appear here (this file is committed to git).
```

`apps/poc-env/app/page.tsx` (client-side, must see PUBLIC_FOO only):

```tsx
'use client';

export const runtime = 'edge';

export default function EnvPoc() {
  return (
    <main style={{ padding: 24 }}>
      <h1>PoC-B · Scenario E · env binding</h1>
      <p>NEXT_PUBLIC_FOO: {process.env.NEXT_PUBLIC_FOO ?? '(not set)'}</p>
      <p>SECRET_KEY (should be undefined here): {String(process.env.SECRET_KEY)}</p>
    </main>
  );
}
```

`apps/poc-env/app/api/secret-probe/route.ts`:

```ts
// Server-side only; SECRET_KEY must reach env.SECRET_KEY in Cloudflare Pages.
export const runtime = 'edge';

export async function GET() {
  // In next-on-pages the request context exposes env via process.env at runtime.
  const value = process.env.SECRET_KEY;
  return Response.json({ hasSecret: typeof value === 'string' && value.length > 0 });
}
```

**Pass criteria**:
1. `NEXT_PUBLIC_FOO` visible in client bundle; `SECRET_KEY` undefined.
2. `/api/secret-probe` returns `{ hasSecret: true }` after `wrangler pages secret put SECRET_KEY` is run.
3. `grep -r SECRET_KEY .next/static/` finds zero hits — secret never reaches the client bundle.
