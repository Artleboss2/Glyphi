import type { ProviderAdapter } from "../provider";
import { ProviderError } from "../provider";
import { metric, number, reading, record, type QuotaReading } from "./quota-shared";

/**
 * The Rust counting proxy, as a source of its own.
 *
 * `glyphi-proxy` runs on loopback and counts every request an agent sends through
 * `http://127.0.0.1:4787/p/<provider>`. Its totals are the only usage some tools
 * will ever produce — a client that writes nothing this app can read still passes
 * through the proxy, so its turn shows up here instead of nowhere.
 *
 * Read, not computed: the proxy owns the counters and this adapter is a window
 * onto `/__glyphi/usage`. Nothing is stored, and a request is made only when the
 * dashboard refreshes.
 */

export const PROXY_ORIGIN = "http://127.0.0.1:4787";
const TIMEOUT_MS = 4_000;
/**
 * The contract allows sixteen metrics per source. Three are the totals and one is
 * reserved for the overflow line, so twelve providers fit by name and everything
 * beyond them is summed rather than dropped: a busy machine can never produce a
 * reading the dashboard rejects.
 */
const TOTAL_METRICS = 3;
const MAX_PROVIDER_METRICS = 12;
const MAX_METRICS = TOTAL_METRICS + MAX_PROVIDER_METRICS + 1;

interface ProxyOptions {
  origin?: string;
  fetch?: typeof fetch;
}

interface DayEntry {
  input: number;
  output: number;
}

function dayOf(value: unknown): string | null {
  const day = record(value).day;
  return typeof day === "string" && /^\d{4}-\d{2}-\d{2}$/u.test(day) ? day : null;
}

function totalsOf(value: unknown): DayEntry {
  const usage = record(value);
  return { input: number(usage.input) ?? 0, output: number(usage.output) ?? 0 };
}

function localDay(instant: Date): string {
  const month = String(instant.getMonth() + 1).padStart(2, "0");
  const day = String(instant.getDate()).padStart(2, "0");
  return `${instant.getFullYear()}-${month}-${day}`;
}

/** Sums today's providers out of a `/__glyphi/usage` payload. */
export function readProxyUsage(payload: unknown, today: string): Map<string, DayEntry> {
  const days = record(payload).days;
  const result = new Map<string, DayEntry>();
  if (!Array.isArray(days)) return result;
  for (const entry of days) {
    if (dayOf(entry) !== today) continue;
    for (const [provider, usage] of Object.entries(record(record(entry).providers))) {
      const totals = totalsOf(usage);
      if (totals.input === 0 && totals.output === 0) continue;
      const existing = result.get(provider) ?? { input: 0, output: 0 };
      result.set(provider, { input: existing.input + totals.input, output: existing.output + totals.output });
    }
  }
  return result;
}

export function createProxyAdapter(options: ProxyOptions = {}): ProviderAdapter {
  const origin = (options.origin ?? PROXY_ORIGIN).replace(/\/+$/u, "");
  const doFetch = options.fetch ?? fetch;

  const read = async (context: { signal: AbortSignal; now: Date }): Promise<QuotaReading> => {
    const signal = AbortSignal.any([context.signal, AbortSignal.timeout(TIMEOUT_MS)]);
    let payload: unknown;
    try {
      const response = await doFetch(`${origin}/__glyphi/usage`, { cache: "no-store", signal });
      if (!response.ok) throw new ProviderError(`http_${response.status}`, "The counting proxy answered with an error.", true);
      payload = await response.json();
    } catch (error) {
      if (error instanceof ProviderError) throw error;
      throw new ProviderError(
        "proxy_unreachable",
        "The counting proxy is not running. It starts with Glyphi; if it was stopped, relaunch the app.",
        true
      );
    }

    const usage = readProxyUsage(payload, localDay(context.now));
    const providers = [...usage.entries()].sort(
      (left, right) => right[1].input + right[1].output - (left[1].input + left[1].output)
    );

    let input = 0;
    let output = 0;
    for (const [, totals] of providers) {
      input += totals.input;
      output += totals.output;
    }

    const metrics = [
      metric("proxy_tokens_today", "Tokens through the proxy today", "tokens", input + output, null, null, null),
      metric("proxy_input_today", "Input through the proxy today", "tokens", input, null, null, null),
      metric("proxy_output_today", "Output through the proxy today", "tokens", output, null, null, null)
    ];
    const named = providers.slice(0, MAX_PROVIDER_METRICS);
    for (const [provider, totals] of named) {
      // A provider name is user data: bound the id and drop what the contract
      // cannot accept instead of failing the whole reading.
      const id = `proxy_${provider}`.slice(0, 64);
      if (!/^[a-z0-9_.-]+$/iu.test(id)) continue;
      metrics.push(metric(id, `${provider.slice(0, 48)} through the proxy`, "tokens", totals.input + totals.output, null, null, null));
    }
    const overflow = providers.slice(MAX_PROVIDER_METRICS);
    if (overflow.length > 0) {
      const others = overflow.reduce((sum, [, totals]) => sum + totals.input + totals.output, 0);
      metrics.push(
        metric(
          "proxy_other",
          `${overflow.length} more ${overflow.length === 1 ? "provider" : "providers"} through the proxy`,
          "tokens",
          others,
          null,
          null,
          null
        )
      );
    }
    // Belt and braces: the reading is trimmed to the contract, never past it.
    metrics.length = Math.min(metrics.length, MAX_METRICS);

    // A proxy that has counted nothing is a real, quiet zero, not a failure: the
    // dashboard should not paint an error on a machine that routes nothing here.
    if (!metrics.some(Boolean)) {
      throw new ProviderError("proxy_idle", "Nothing has gone through the counting proxy today.", false);
    }
    return reading("Counting proxy", metrics);
  };

  return {
    id: "proxy",
    name: "Counting proxy",
    isAvailable: async () => {
      try {
        const response = await doFetch(`${origin}/__glyphi/health`, {
          cache: "no-store",
          signal: AbortSignal.timeout(1_500)
        });
        await response.body?.cancel();
        return response.ok;
      } catch {
        return false;
      }
    },
    async refresh(context) {
      const reading = await read(context);
      return {
        ...reading,
        accountKey: "loopback",
        source: "provider_quota",
        analytics: null,
        error: null,
        updatedAt: context.now.toISOString()
      };
    }
  };
}
