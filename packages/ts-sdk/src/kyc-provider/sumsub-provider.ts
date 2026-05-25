import type {
  KycErrorCode,
  KycEventListener,
  KycIdentityProvider,
  KycInlineOutcome,
  KycSessionEvent,
  KycSessionHandle,
  KycSessionStartParams,
  KycStep,
} from "./types.js";
import { KycSdkError } from "./types.js";

const DEFAULT_SUMSUB_SCRIPT_URL =
  "https://static.sumsub.com/idensic/static/sns-websdk-builder.js";

export interface RealSumsubProviderOptions {
  scriptUrl?: string;
  tokenRefresh?: (params: KycSessionStartParams) => Promise<string>;
}

interface SumsubSdkGlobal {
  init(
    accessToken: string,
    tokenRefresh: () => Promise<string>,
  ): SumsubSdkBuilder;
}

interface SumsubSdkBuilder {
  withConf(config: SumsubSdkConfig): SumsubSdkBuilder;
  withOptions(options: SumsubSdkOptions): SumsubSdkBuilder;
  on(eventName: string, handler: (payload: unknown) => void): SumsubSdkBuilder;
  onMessage(
    handler: (type: string, payload: unknown) => void,
  ): SumsubSdkBuilder;
  build(): SumsubSdkInstance;
}

interface SumsubSdkInstance {
  launch(target: string | HTMLElement): void;
  destroy?: () => void;
}

interface SumsubSdkConfig {
  lang: string;
  country: string;
}

interface SumsubSdkOptions {
  addViewportTag: boolean;
  adaptIframeHeight: boolean;
}

interface GlobalWithSumsub {
  snsWebSdk?: SumsubSdkGlobal;
}

let scriptLoadPromise: Promise<void> | undefined;

export function createRealSumsubProvider(
  options: RealSumsubProviderOptions = {},
): KycIdentityProvider {
  return {
    name: "sumsub",
    async mount(target, params) {
      return mountSumsubWebSdk(target, params, options);
    },
  };
}

async function mountSumsubWebSdk(
  target: HTMLElement,
  params: KycSessionStartParams,
  options: RealSumsubProviderOptions,
): Promise<KycSessionHandle> {
  if (typeof document === "undefined") {
    throw new KycSdkError(
      "browser_unsupported",
      "SumSub WebSDK requires a browser document.",
    );
  }

  await ensureSumsubScript(options.scriptUrl ?? DEFAULT_SUMSUB_SCRIPT_URL);
  const sdk = (globalThis as GlobalWithSumsub).snsWebSdk;
  if (!sdk) {
    throw new KycSdkError(
      "sdk_load_failed",
      "SumSub WebSDK did not initialize.",
    );
  }

  const session = createEventSession();
  const instance = sdk
    .init(params.sdkToken, async () =>
      options.tokenRefresh ? options.tokenRefresh(params) : params.sdkToken,
    )
    .withConf({
      lang: params.locale,
      country: params.country,
    })
    .withOptions({
      addViewportTag: false,
      adaptIframeHeight: true,
    })
    .onMessage((type, payload) => {
      const event = mapSumsubMessage(type, payload);
      if (event) {
        session.emit(event);
      }
    })
    .build();

  instance.launch(targetSelector(target));

  return {
    unmount() {
      session.close();
      instance.destroy?.();
      if (typeof target.replaceChildren === "function") {
        target.replaceChildren();
      }
    },
    subscribe(listener) {
      return session.subscribe(listener);
    },
  };
}

function ensureSumsubScript(scriptUrl: string): Promise<void> {
  if ((globalThis as GlobalWithSumsub).snsWebSdk) {
    return Promise.resolve();
  }
  if (scriptLoadPromise) {
    return scriptLoadPromise.then(() => {
      if ((globalThis as GlobalWithSumsub).snsWebSdk) {
        return;
      }
      scriptLoadPromise = undefined;
      return ensureSumsubScript(scriptUrl);
    });
  }
  scriptLoadPromise = new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = scriptUrl;
    script.async = true;
    script.onload = () => resolve();
    script.onerror = () => {
      scriptLoadPromise = undefined;
      reject(
        new KycSdkError(
          "sdk_load_failed",
          "SumSub WebSDK script failed to load.",
        ),
      );
    };
    document.head.appendChild(script);
  });
  return scriptLoadPromise;
}

function targetSelector(target: HTMLElement): string | HTMLElement {
  if (target.id) {
    return `#${target.id}`;
  }
  return target;
}

function createEventSession(): {
  close: () => void;
  emit: (event: KycSessionEvent) => void;
  subscribe: (listener: KycEventListener) => () => void;
} {
  let closed = false;
  let readyFired = false;
  const listeners = new Set<KycEventListener>();

  return {
    close() {
      closed = true;
      listeners.clear();
    },
    emit(event) {
      if (closed) {
        return;
      }
      if (event.kind === "ready") {
        readyFired = true;
      }
      for (const listener of listeners) {
        listener(event);
      }
    },
    subscribe(listener) {
      if (closed) {
        return () => {};
      }
      listeners.add(listener);
      if (readyFired) {
        listener({ kind: "ready" });
      }
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

function mapSumsubMessage(
  type: string,
  payload: unknown,
): KycSessionEvent | null {
  switch (type) {
    case "idCheck.onReady":
    case "idCheck.onInitialized":
      return { kind: "ready" };
    case "idCheck.onApplicantSubmitted":
      return { kind: "submitted" };
    case "idCheck.onApplicantVerificationCompleted":
      return {
        kind: "completed",
        outcome: mapVerificationOutcome(payload),
      };
    case "idCheck.onStepInitiated":
    case "idCheck.onStepCompleted": {
      const step = mapStep(payload);
      return step ? { kind: "step_changed", step } : null;
    }
    case "idCheck.onApplicantResubmitted":
      return { kind: "submitted" };
    case "idCheck.onApplicantLoaded":
      return null;
    case "idCheck.onError": {
      const error = mapError(payload);
      return {
        kind: "error",
        code: error.code,
        recoverable: error.recoverable,
      };
    }
    default:
      return null;
  }
}

function mapVerificationOutcome(payload: unknown): KycInlineOutcome {
  const answer = stringField(payload, ["reviewResult", "reviewAnswer"]);
  if (answer === "GREEN") {
    return "approved";
  }
  if (answer === "RED") {
    return "rejected";
  }
  return "pending_review";
}

function mapStep(payload: unknown): KycStep | null {
  const step =
    stringField(payload, ["idDocSetType"]) ?? stringField(payload, ["step"]);
  if (!step) {
    return null;
  }
  const normalized = step.toLowerCase();
  if (normalized.includes("selfie") || normalized.includes("video")) {
    return "selfie";
  }
  if (normalized.includes("liveness")) {
    return "liveness";
  }
  if (normalized.includes("address")) {
    return "address";
  }
  if (normalized.includes("questionnaire") || normalized.includes("consent")) {
    return "consent";
  }
  if (normalized.includes("review")) {
    return "review";
  }
  return "document";
}

function mapError(payload: unknown): {
  code: KycErrorCode;
  recoverable: boolean;
} {
  const code = `${stringField(payload, ["code"]) ?? ""} ${
    stringField(payload, ["reason"]) ?? ""
  }`.toLowerCase();
  if (code.includes("token")) {
    return { code: "sdk_token_expired", recoverable: true };
  }
  if (code.includes("permission") || code.includes("camera")) {
    return { code: "permission_denied", recoverable: true };
  }
  if (code.includes("network")) {
    return { code: "network_lost", recoverable: true };
  }
  if (code.includes("init") || code.includes("load")) {
    return { code: "sdk_load_failed", recoverable: true };
  }
  return { code: "unknown", recoverable: true };
}

function stringField(payload: unknown, path: string[]): string | null {
  let current: unknown = payload;
  for (const part of path) {
    if (!current || typeof current !== "object") {
      return null;
    }
    current = (current as Record<string, unknown>)[part];
  }
  return typeof current === "string" ? current : null;
}
