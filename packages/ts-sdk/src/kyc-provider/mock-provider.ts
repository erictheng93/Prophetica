import type {
  KycEventListener,
  KycIdentityProvider,
  KycInlineOutcome,
  KycSessionEvent,
  KycSessionHandle,
  KycSessionStartParams,
} from "./types.js";

export interface MockKycProviderOptions {
  autoSubmitAfterMs?: number;
  globalHookKey?: string;
}

export function createMockKycProvider(
  options: MockKycProviderOptions = {},
): KycIdentityProvider {
  return {
    name: "mock",
    async mount(target, params) {
      return mountInternal(target, params, options);
    },
  };
}

interface InternalSession {
  unmounted: boolean;
  readyFired: boolean;
  listeners: Set<KycEventListener>;
  autoSubmitTimer?: ReturnType<typeof setTimeout>;
  detachGlobalHook?: () => void;
}

function mountInternal(
  target: HTMLElement,
  params: KycSessionStartParams,
  options: MockKycProviderOptions,
): KycSessionHandle {
  const session: InternalSession = {
    unmounted: false,
    readyFired: false,
    listeners: new Set(),
  };
  const autoSubmitMs = options.autoSubmitAfterMs ?? 5000;
  const globalHookKey = options.globalHookKey ?? "__mockKycComplete";

  renderStub(target, params);

  const emit = (event: KycSessionEvent): void => {
    if (session.unmounted) {
      return;
    }
    if (event.kind === "ready") {
      session.readyFired = true;
    }
    for (const listener of session.listeners) {
      listener(event);
    }
  };

  // `ready` is sticky: late subscribers replay it on attach. This guards
  // against the common pattern `await mount(); subscribe(...)` where the
  // microtask resolving `mount`'s promise drains after a microtask-scheduled
  // emit would have already fired into an empty listener set.
  queueMicrotask(() => emit({ kind: "ready" }));

  if (autoSubmitMs > 0) {
    session.autoSubmitTimer = setTimeout(() => {
      emit({ kind: "submitted" });
    }, autoSubmitMs);
  }

  session.detachGlobalHook = attachGlobalHook(globalHookKey, (outcome) => {
    emit({ kind: "completed", outcome });
  });

  return {
    unmount: () => {
      if (session.unmounted) {
        return;
      }
      session.unmounted = true;
      session.listeners.clear();
      if (session.autoSubmitTimer !== undefined) {
        clearTimeout(session.autoSubmitTimer);
      }
      session.detachGlobalHook?.();
      clearStub(target);
    },
    subscribe: (listener) => {
      if (session.unmounted) {
        return () => {};
      }
      session.listeners.add(listener);
      if (session.readyFired) {
        listener({ kind: "ready" });
      }
      return () => {
        session.listeners.delete(listener);
      };
    },
  };
}

function renderStub(target: HTMLElement, params: KycSessionStartParams): void {
  if (typeof document === "undefined") {
    return;
  }
  if (typeof target.replaceChildren !== "function") {
    return;
  }
  const wrapper = document.createElement("div");
  wrapper.setAttribute("data-testid", "kyc-mock-stub");
  wrapper.textContent = `MOCK KYC: ${params.targetTier} · ${params.locale}`;
  target.replaceChildren(wrapper);
}

function clearStub(target: HTMLElement): void {
  if (typeof target.replaceChildren === "function") {
    target.replaceChildren();
  }
}

type GlobalHookHost = Record<string, unknown>;

function attachGlobalHook(
  key: string,
  handler: (outcome: KycInlineOutcome) => void,
): () => void {
  const host = globalThis as GlobalHookHost;
  const previous = host[key];
  host[key] = handler;
  return () => {
    if (host[key] !== handler) {
      return;
    }
    if (previous === undefined) {
      delete host[key];
    } else {
      host[key] = previous;
    }
  };
}
