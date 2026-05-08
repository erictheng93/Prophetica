import assert from "node:assert/strict";
import { test } from "node:test";

import {
  createMockKycProvider,
  type KycSessionStartParams,
} from "../src/kyc-provider/index.js";

// SPEC-005a §3.3 — mock provider must render a visible MOCK-tagged stub
// (`<div data-testid="kyc-mock-stub">`) inside the target element AND
// must not touch the network. The default kyc-provider.test.ts uses an
// `({} as HTMLElement)` fake which silently skips the render path; this
// file installs a minimal DOM stub so the render path is actually exercised.

const params: KycSessionStartParams = {
  sdkToken: "mock_dom_test",
  targetTier: "TIER_2",
  locale: "zh-TW",
  country: "TW",
};

interface CapturedChild {
  tagName: string;
  testId: string | null;
  textContent: string;
}

interface DomTarget {
  target: HTMLElement;
  children: CapturedChild[];
}

function installDomStub(): {
  cleanup: () => void;
  createTarget: () => DomTarget;
} {
  const previousDocument = (globalThis as { document?: unknown }).document;

  (globalThis as { document?: unknown }).document = {
    createElement(tag: string) {
      const attrs: Record<string, string> = {};
      const node = {
        tagName: tag.toUpperCase(),
        setAttribute(name: string, value: string) {
          attrs[name] = value;
        },
        getAttribute(name: string): string | null {
          return Object.prototype.hasOwnProperty.call(attrs, name)
            ? attrs[name] ?? null
            : null;
        },
        textContent: "",
      };
      return node;
    },
  };

  return {
    cleanup() {
      if (previousDocument === undefined) {
        delete (globalThis as { document?: unknown }).document;
      } else {
        (globalThis as { document?: unknown }).document = previousDocument;
      }
    },
    createTarget(): DomTarget {
      const captured: CapturedChild[] = [];
      const target = {
        replaceChildren(...elements: unknown[]) {
          captured.length = 0;
          for (const element of elements) {
            const e = element as {
              tagName?: string;
              getAttribute?: (k: string) => string | null;
              textContent?: string;
            };
            captured.push({
              tagName: e.tagName ?? "",
              testId: e.getAttribute ? e.getAttribute("data-testid") : null,
              textContent: e.textContent ?? "",
            });
          }
        },
      } as unknown as HTMLElement;
      return { target, children: captured };
    },
  };
}

test("mock provider renders <div data-testid='kyc-mock-stub'> into target", async () => {
  const dom = installDomStub();
  try {
    const { target, children } = dom.createTarget();
    const provider = createMockKycProvider({ autoSubmitAfterMs: 0 });
    const handle = await provider.mount(target, params);

    assert.equal(children.length, 1, "mount should render exactly one child");
    assert.equal(children[0]?.tagName, "DIV");
    assert.equal(children[0]?.testId, "kyc-mock-stub");
    // Stub text contains both the tier and locale so dev visually sees what
    // sdk shape would have been delivered.
    assert.match(children[0]?.textContent ?? "", /TIER_2/);
    assert.match(children[0]?.textContent ?? "", /zh-TW/);

    handle.unmount();
    assert.equal(children.length, 0, "unmount should clear rendered children");
  } finally {
    dom.cleanup();
  }
});

test("mock provider lifecycle does NOT touch globalThis.fetch", async () => {
  const dom = installDomStub();
  const previousFetch = (globalThis as { fetch?: unknown }).fetch;
  let fetchCallCount = 0;
  (globalThis as { fetch?: unknown }).fetch = async () => {
    fetchCallCount += 1;
    return new Response(null);
  };

  try {
    const { target } = dom.createTarget();
    const provider = createMockKycProvider({
      autoSubmitAfterMs: 5,
      globalHookKey: "__mockKycComplete_no_fetch",
    });
    const handle = await provider.mount(target, params);
    handle.subscribe(() => {
      // listener body is intentionally empty; we only care whether the
      // mock provider itself reached for fetch.
    });

    await new Promise((resolve) => setTimeout(resolve, 30));
    handle.unmount();

    assert.equal(
      fetchCallCount,
      0,
      "SPEC-005a §3.3 forbids the mock from making external network calls",
    );
  } finally {
    if (previousFetch === undefined) {
      delete (globalThis as { fetch?: unknown }).fetch;
    } else {
      (globalThis as { fetch?: unknown }).fetch = previousFetch;
    }
    dom.cleanup();
  }
});
