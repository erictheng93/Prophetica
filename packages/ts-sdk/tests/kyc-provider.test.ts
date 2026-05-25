import assert from "node:assert/strict";
import { test } from "node:test";

import {
  KycSdkError,
  createMockKycProvider,
  createIproovMockProvider,
  createRealIproovProvider,
  createRealSumsubProvider,
  selectKycProvider,
  type KycInlineOutcome,
  type KycSessionEvent,
} from "../src/kyc-provider/index.js";

const fakeTarget = (): HTMLElement => ({}) as unknown as HTMLElement;

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

test("mock provider emits ready then submitted via auto-submit", async () => {
  const provider = createMockKycProvider({ autoSubmitAfterMs: 10 });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_2",
    locale: "zh-TW",
    country: "TW",
  });

  const events: KycSessionEvent[] = [];
  handle.subscribe((event) => events.push(event));

  await wait(40);

  assert.equal(events[0]?.kind, "ready");
  assert.ok(
    events.some((e) => e.kind === "submitted"),
    "expected a submitted event before deadline",
  );

  handle.unmount();
});

test("mock provider unmount stops further events and is idempotent", async () => {
  const provider = createMockKycProvider({
    autoSubmitAfterMs: 50,
    globalHookKey: "__mockKycComplete_idempotent",
  });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_2",
    locale: "en",
    country: "TW",
  });

  const events: KycSessionEvent[] = [];
  handle.subscribe((event) => events.push(event));

  handle.unmount();
  handle.unmount();
  await wait(80);

  assert.ok(
    events.every((e) => e.kind !== "submitted"),
    "submitted must not fire after unmount",
  );
});

test("unsubscribe stops the listener from receiving further events", async () => {
  const provider = createMockKycProvider({
    autoSubmitAfterMs: 20,
    globalHookKey: "__mockKycComplete_unsub",
  });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_2",
    locale: "en",
    country: "TW",
  });

  const events: KycSessionEvent[] = [];
  const unsubscribe = handle.subscribe((event) => events.push(event));
  unsubscribe();
  await wait(60);

  assert.ok(
    events.every((e) => e.kind !== "submitted"),
    "submitted must not reach the listener after unsubscribe",
  );

  handle.unmount();
});

test("global hook emits a completed event with the supplied outcome", async () => {
  const hookKey = "__mockKycComplete_global";
  const provider = createMockKycProvider({
    autoSubmitAfterMs: 0,
    globalHookKey: hookKey,
  });
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "mock_test",
    targetTier: "TIER_3",
    locale: "ja-JP",
    country: "JP",
  });

  const events: KycSessionEvent[] = [];
  handle.subscribe((event) => events.push(event));

  await wait(5);

  type Hook = (outcome: KycInlineOutcome) => void;
  const host = globalThis as Record<string, unknown>;
  const hook = host[hookKey] as Hook | undefined;
  assert.equal(typeof hook, "function", "global hook should be installed");
  hook?.("approved");

  const completed = events.find((e) => e.kind === "completed");
  assert.equal(completed?.kind, "completed");
  if (completed?.kind === "completed") {
    assert.equal(completed.outcome, "approved");
  }

  handle.unmount();

  assert.equal(
    host[hookKey],
    undefined,
    "global hook should be removed after unmount",
  );
});

test("selectKycProvider resolves provider by sdk token prefix", () => {
  const sandbox = selectKycProvider({
    sdkToken: "sumsub_sandbox_user_42_TIER_2_1",
  });
  assert.deepEqual(sandbox, {
    mountStrategy: "sequential",
    stages: [{ name: "sumsub", required: true, continueAfterSubmit: false }],
  });

  const real = selectKycProvider({ sdkToken: "sumsub_real_token_xyz" });
  assert.deepEqual(real, {
    mountStrategy: "sequential",
    stages: [{ name: "sumsub", required: true, continueAfterSubmit: false }],
  });

  const mock = selectKycProvider({ sdkToken: "mock_local_dev" });
  assert.deepEqual(mock, {
    mountStrategy: "sequential",
    stages: [{ name: "mock", required: true, continueAfterSubmit: false }],
  });
});

test("selectKycProvider resolves Tier 3 SumSub sandbox token to a two-stage chain", () => {
  const chain = selectKycProvider({
    sdkToken: "sumsub_sandbox_user_42_TIER_3_1",
  });

  assert.deepEqual(chain, {
    mountStrategy: "sequential",
    stages: [
      { name: "sumsub", required: true, continueAfterSubmit: true },
      { name: "iproov", required: true, continueAfterSubmit: false },
    ],
  });
});

test("selectKycProvider throws KycSdkError on unknown prefix", () => {
  assert.throws(
    () => selectKycProvider({ sdkToken: "onfido_xxx" }),
    (err: unknown) =>
      err instanceof KycSdkError && err.code === "unknown_sdk_token_prefix",
  );
});

test("real SumSub provider loads WebSDK, launches into target, and maps messages", async () => {
  const dom = installSumsubWebSdkStub();
  try {
    const provider = createRealSumsubProvider({
      scriptUrl: "https://static.sumsub.test/sdk.js",
      tokenRefresh: async () => "sumsub_real_refreshed",
    });
    const target = dom.createTarget("sumsub-container");
    const handle = await provider.mount(target, {
      sdkToken: "sumsub_real_xyz",
      targetTier: "TIER_2",
      locale: "zh-TW",
      country: "TW",
    });

    const events: KycSessionEvent[] = [];
    handle.subscribe((event) => events.push(event));

    assert.equal(
      dom.appendedScripts[0]?.src,
      "https://static.sumsub.test/sdk.js",
    );
    assert.equal(dom.initCalls[0]?.accessToken, "sumsub_real_xyz");
    assert.equal(
      await dom.initCalls[0]?.tokenRefresh(),
      "sumsub_real_refreshed",
    );
    assert.deepEqual(dom.configs[0], {
      lang: "zh-TW",
      country: "TW",
    });
    assert.deepEqual(dom.options[0], {
      addViewportTag: false,
      adaptIframeHeight: true,
    });
    assert.equal(dom.launchTargets[0], "#sumsub-container");

    dom.emit("idCheck.onReady", {});
    dom.emit("idCheck.onApplicantSubmitted", {});
    dom.emit("idCheck.onApplicantVerificationCompleted", {
      reviewResult: { reviewAnswer: "GREEN" },
    });

    assert.deepEqual(events, [
      { kind: "ready" },
      { kind: "submitted" },
      { kind: "completed", outcome: "approved" },
    ]);

    handle.unmount();
    assert.equal(dom.destroyCallCount, 1);
  } finally {
    dom.cleanup();
  }
});

test("real SumSub provider maps WebSDK errors", async () => {
  const dom = installSumsubWebSdkStub();
  try {
    const provider = createRealSumsubProvider({
      scriptUrl: "https://static.sumsub.test/sdk.js",
    });
    const handle = await provider.mount(dom.createTarget("sumsub-error"), {
      sdkToken: "sumsub_real_xyz",
      targetTier: "TIER_2",
      locale: "en",
      country: "TW",
    });
    const events: KycSessionEvent[] = [];
    handle.subscribe((event) => events.push(event));

    dom.emit("idCheck.onError", { code: "invalid-token" });
    dom.emit("idCheck.onError", {
      code: "camera-error",
      reason: "permissionsDenied",
    });
    dom.emit("idCheck.onError", { code: "initialization-error" });

    assert.deepEqual(events, [
      { kind: "error", code: "sdk_token_expired", recoverable: true },
      { kind: "error", code: "permission_denied", recoverable: true },
      { kind: "error", code: "sdk_load_failed", recoverable: true },
    ]);
    handle.unmount();
  } finally {
    dom.cleanup();
  }
});

test("real SumSub provider rejects outside browser runtime", async () => {
  const previousDocument = (globalThis as { document?: unknown }).document;
  const previousSdk = (globalThis as { snsWebSdk?: unknown }).snsWebSdk;
  delete (globalThis as { document?: unknown }).document;
  delete (globalThis as { snsWebSdk?: unknown }).snsWebSdk;
  const provider = createRealSumsubProvider();
  try {
    await assert.rejects(
      () =>
        provider.mount(fakeTarget(), {
          sdkToken: "sumsub_real_xyz",
          targetTier: "TIER_2",
          locale: "en",
          country: "TW",
        }),
      (err: unknown) =>
        err instanceof KycSdkError && err.code === "browser_unsupported",
    );
  } finally {
    if (previousDocument !== undefined) {
      (globalThis as { document?: unknown }).document = previousDocument;
    }
    if (previousSdk !== undefined) {
      (globalThis as { snsWebSdk?: unknown }).snsWebSdk = previousSdk;
    }
  }
});

test("override option short-circuits sdk token resolution", () => {
  const stub = createMockKycProvider();
  const resolved = selectKycProvider({
    sdkToken: "anything_unsupported",
    override: stub,
  });
  assert.deepEqual(resolved, {
    mountStrategy: "sequential",
    stages: [{ name: "mock", required: true, continueAfterSubmit: false }],
  });
});

test("real iProov provider mount throws not_implemented", async () => {
  const realProvider = createRealIproovProvider();
  assert.equal(realProvider.name, "iproov");
  await assert.rejects(
    () =>
      realProvider.mount(fakeTarget(), {
        sdkToken: "iproov_real_xyz",
        targetTier: "TIER_3",
        locale: "en",
        country: "TW",
      }),
    (err: unknown) =>
      err instanceof KycSdkError && err.code === "not_implemented",
  );
});

test("iProov mock provider emits ready and submitted", async () => {
  const provider = createIproovMockProvider();
  const handle = await provider.mount(fakeTarget(), {
    sdkToken: "iproov_mock_xyz",
    targetTier: "TIER_3",
    locale: "en",
    country: "TW",
  });
  const events: KycSessionEvent[] = [];
  handle.subscribe((event) => events.push(event));
  await wait(15);
  assert.equal(events[0]?.kind, "ready");
  assert.ok(events.some((e) => e.kind === "submitted"));
  handle.unmount();
});

interface ScriptStub {
  src: string;
  async: boolean;
  onload: (() => void) | null;
  onerror: (() => void) | null;
}

interface TargetStub {
  id: string;
  replaceChildren(): void;
}

type SumsubMessageHandler = (type: string, payload: unknown) => void;

function installSumsubWebSdkStub(): {
  appendedScripts: ScriptStub[];
  configs: unknown[];
  options: unknown[];
  initCalls: Array<{
    accessToken: string;
    tokenRefresh: () => Promise<string>;
  }>;
  launchTargets: unknown[];
  get destroyCallCount(): number;
  cleanup: () => void;
  createTarget: (id: string) => HTMLElement;
  emit: (type: string, payload: unknown) => void;
} {
  const host = globalThis as {
    document?: unknown;
    snsWebSdk?: unknown;
  };
  const previousDocument = host.document;
  const previousSdk = host.snsWebSdk;
  const appendedScripts: ScriptStub[] = [];
  const configs: unknown[] = [];
  const options: unknown[] = [];
  const initCalls: Array<{
    accessToken: string;
    tokenRefresh: () => Promise<string>;
  }> = [];
  const launchTargets: unknown[] = [];
  const handlers: SumsubMessageHandler[] = [];
  let destroyCallCount = 0;

  const sdk = {
    init(accessToken: string, tokenRefresh: () => Promise<string>) {
      initCalls.push({ accessToken, tokenRefresh });
      return {
        withConf(config: unknown) {
          configs.push(config);
          return this;
        },
        withOptions(option: unknown) {
          options.push(option);
          return this;
        },
        on() {
          return this;
        },
        onMessage(handler: SumsubMessageHandler) {
          handlers.push(handler);
          return this;
        },
        build() {
          return {
            launch(target: unknown) {
              launchTargets.push(target);
            },
            destroy() {
              destroyCallCount += 1;
            },
          };
        },
      };
    },
  };

  host.document = {
    createElement(tagName: string) {
      assert.equal(tagName, "script");
      return {
        src: "",
        async: false,
        onload: null,
        onerror: null,
      } satisfies ScriptStub;
    },
    head: {
      appendChild(script: ScriptStub) {
        appendedScripts.push(script);
        queueMicrotask(() => {
          host.snsWebSdk = sdk;
          script.onload?.();
        });
      },
    },
  };

  return {
    appendedScripts,
    configs,
    options,
    initCalls,
    launchTargets,
    get destroyCallCount() {
      return destroyCallCount;
    },
    cleanup() {
      if (previousDocument === undefined) {
        delete host.document;
      } else {
        host.document = previousDocument;
      }
      if (previousSdk === undefined) {
        delete host.snsWebSdk;
      } else {
        host.snsWebSdk = previousSdk;
      }
    },
    createTarget(id: string): HTMLElement {
      return {
        id,
        replaceChildren() {
          // no-op test target
        },
      } satisfies TargetStub as unknown as HTMLElement;
    },
    emit(type: string, payload: unknown) {
      for (const handler of handlers) {
        handler(type, payload);
      }
    },
  };
}
