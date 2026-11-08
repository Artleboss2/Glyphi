import type { CurrencyCode } from "./desktop-api";

/**
 * Reference exchange rates, in display-currency units per US dollar. They ship
 * with the app instead of being typed in Settings: a cost estimate should never
 * drift because someone fat-fingered a rate, and nothing here needs a network
 * call. `RATE_TABLE_DATE` is the day the snapshot was taken.
 */
export const RATE_TABLE_DATE = "2026-11-01";

export const CURRENCY_RATES: Record<CurrencyCode, number> = {
  USD: 1,
  EUR: 0.92,
  GBP: 0.78,
  CAD: 1.39,
  JPY: 152,
  CHF: 0.89
};

/** Units of `currency` for one US dollar. */
export function currencyRate(currency: CurrencyCode): number {
  return CURRENCY_RATES[currency] ?? 1;
}
