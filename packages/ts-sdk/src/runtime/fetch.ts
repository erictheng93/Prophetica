import { decodeErrorEnvelope } from "./errors.js";

export type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export interface AuthOptions {
  bearerToken?: string;
  userId?: string;
}

export interface ClientOptions {
  baseUrl: string;
  auth?: AuthOptions;
  fetcher?: Fetcher;
}

export interface JsonRequestOptions {
  method?: "GET" | "POST";
  path: string;
  query?: URLSearchParams;
  body?: unknown;
}

export class HttpClient {
  private readonly baseUrl: string;
  private readonly auth?: AuthOptions;
  private readonly fetcher: Fetcher;

  constructor(options: ClientOptions) {
    this.baseUrl = trimTrailingSlash(options.baseUrl);
    this.auth = options.auth;
    this.fetcher = options.fetcher ?? fetch;
  }

  async json<T>(options: JsonRequestOptions): Promise<T> {
    const url = new URL(`${this.baseUrl}${options.path}`);
    if (options.query) {
      url.search = options.query.toString();
    }

    const headers = new Headers({
      accept: "application/json",
    });
    if (options.body !== undefined) {
      headers.set("content-type", "application/json");
    }
    if (this.auth?.bearerToken) {
      headers.set("authorization", `Bearer ${this.auth.bearerToken}`);
    }
    if (this.auth?.userId) {
      headers.set("x-user-id", this.auth.userId);
    }

    const response = await this.fetcher(url.toString(), {
      method: options.method ?? "GET",
      cache: "no-store",
      headers,
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
    });
    const body = await readJson(response);

    if (!response.ok) {
      throw decodeErrorEnvelope(response.status, body);
    }

    return body as T;
  }
}

async function readJson(response: Response): Promise<unknown> {
  const text = await response.text();
  if (text.length === 0) {
    return undefined;
  }

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return { message: text };
  }
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
