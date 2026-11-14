import { describe, expect, it } from "vitest";
import { createProxyAdapter, readProxyUsage } from "./proxy";
import { ProviderError } from "../provider";

/** Noon local time, so the day the adapter builds is the day in the payload. */
const now = new Date(2026, 10, 14, 12, 0, 0);
const today = "2026-11-14";
const context = { now, signal: new AbortController().signal } as never;

function payload(days: Array<{ day: string; providers: Record<string, { input: number; output: number }> }>) {
  return { days, totals: {} };
}

function respond(body: unknown, status = 200): typeof fetch {
  return (async () =>
    new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } })) as unknown as typeof fetch;
}

function failing(): typeof fetch {
  return (async () => {
    throw new Error("connect ECONNREFUSED");
  }) as unknown as typeof fetch;
}

const used = (provider: { quotaMetrics?: Array<{ id: string; used: number | null }> | null }, id: string) =>
  provider.quotaMetrics?.find((metric) => metric.id === id)?.used ?? null;

describe("readProxyUsage", () => {
  it("keeps only the requested day and sums each provider", () => {
    const usage = readProxyUsage(
      payload([
        { day: today, providers: { openai: { input: 10, output: 5 }, anthropic: { input: 2, output: 1 } } },
        { day: "2026-11-13", providers: { openai: { input: 999, output: 999 } } }
      ]),
      today
    );

    expect([...usage.entries()]).toEqual([
      ["openai", { input: 10, output: 5 }],
      ["anthropic", { input: 2, output: 1 }]
    ]);
  });

  it("ignores a payload with no days", () => {
    expect(readProxyUsage({}, today).size).toBe(0);
  });
});

describe("createProxyAdapter", () => {
  it("reports today's input, output and total", async () => {
    const adapter = createProxyAdapter({
      fetch: respond(
        payload([
          {
            day: today,
            providers: {
              openai: { input: 900, output: 150 },
              anthropic: { input: 320, output: 80 }
            }
          }
        ])
      )
    });

    const provider = await adapter.refresh(context);

    expect(used(provider, "proxy_tokens_today")).toBe(1450);
    expect(used(provider, "proxy_input_today")).toBe(1220);
    expect(used(provider, "proxy_output_today")).toBe(230);
    expect(provider.error).toBeNull();
    expect(provider.accountKey).toBe("loopback");
  });

  it("names the busiest providers first", async () => {
    const adapter = createProxyAdapter({
      fetch: respond(
        payload([
          {
            day: today,
            providers: {
              small: { input: 1, output: 0 },
              big: { input: 5000, output: 0 },
              middle: { input: 40, output: 0 }
            }
          }
        ])
      )
    });

    const provider = await adapter.refresh(context);
    const named = provider.quotaMetrics?.slice(3).map((metric) => metric.id);

    expect(named).toEqual(["proxy_big", "proxy_middle", "proxy_small"]);
  });

  it("sums providers past the twelfth into one line instead of breaking the contract", async () => {
    const providers: Record<string, { input: number; output: number }> = {};
    for (let index = 0; index < 40; index += 1) {
      // Descending volumes, so the twelve biggest are the ones with the lowest index.
      providers[`provider-${index}`] = { input: 1000 - index * 10, output: 0 };
    }

    const adapter = createProxyAdapter({ fetch: respond(payload([{ day: today, providers }])) });
    const provider = await adapter.refresh(context);
    const metrics = provider.quotaMetrics ?? [];

    expect(metrics).toHaveLength(16);
    expect(new Set(metrics.map((metric) => metric.id)).size).toBe(16);
    expect(metrics.some((metric) => metric.id === "proxy_other")).toBe(true);

    const overflow = Object.values(providers)
      .slice(12)
      .reduce((sum, usage) => sum + usage.input + usage.output, 0);
    expect(used(provider, "proxy_other")).toBe(overflow);
    // The totals still describe everything, not only the named twelve.
    const all = Object.values(providers).reduce((sum, usage) => sum + usage.input + usage.output, 0);
    expect(used(provider, "proxy_tokens_today")).toBe(all);
  });

  it("never returns more than sixteen metrics even with odd provider names", async () => {
    const providers: Record<string, { input: number; output: number }> = {
      "with space": { input: 5, output: 0 },
      "Ünïcode": { input: 5, output: 0 },
      "ok_name": { input: 5, output: 0 }
    };
    for (let index = 0; index < 30; index += 1) providers[`p${index}`] = { input: 1, output: 1 };

    const adapter = createProxyAdapter({ fetch: respond(payload([{ day: today, providers }])) });
    const metrics = (await adapter.refresh(context)).quotaMetrics ?? [];

    expect(metrics.length).toBeLessThanOrEqual(16);
    expect(new Set(metrics.map((metric) => metric.id)).size).toBe(metrics.length);
  });

  it("raises a provider error when the proxy answers with a failure", async () => {
    const adapter = createProxyAdapter({ fetch: respond({ error: "boom" }, 500) });

    await expect(adapter.refresh(context)).rejects.toMatchObject({ code: "http_500" });
  });

  it("raises a provider error when nothing is listening", async () => {
    const adapter = createProxyAdapter({ fetch: failing() });

    await expect(adapter.refresh(context)).rejects.toBeInstanceOf(ProviderError);
    await expect(adapter.refresh(context)).rejects.toMatchObject({ code: "proxy_unreachable" });
  });

  it("reports availability from the health endpoint", async () => {
    const up = createProxyAdapter({ fetch: respond({ ok: true }) });
    const down = createProxyAdapter({ fetch: failing() });

    await expect(up.isAvailable()).resolves.toBe(true);
    await expect(down.isAvailable()).resolves.toBe(false);
  });
});
