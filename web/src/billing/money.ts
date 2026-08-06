// Reading and writing the two-decimal quantities a billing form takes: a unit
// price (integer cents) and a VAT rate (integer basis points). Both are
// "hundredths of something", so one parser and one formatter serve both --
// there is no second, slightly different rule to drift.
//
// This file does NOT compute money. Totals, VAT breakdowns and everything else
// derived come from the server (`docs/design/billing.md`), so the client can
// never disagree with the document. What lives here is strictly the edge where
// a human types "1.234,56" and the API wants `123456`, and back again.
//
// The parse rule is deliberately locale-independent, because a Dutch user with
// an English UI still types Dutch numbers:
//   - whitespace (including the non-breaking and thin spaces a paste from a
//     spreadsheet carries) is a grouping separator and is dropped;
//   - if BOTH `.` and `,` appear, the last one is the decimal separator;
//   - if only one kind appears, exactly once, followed by one or two digits,
//     it is the decimal separator ("1,50" and "1.50" both mean one and a half);
//   - otherwise every separator is a grouping separator, and the integer part
//     must really be grouped in threes ("1.500" and "1,500" both mean fifteen
//     hundred; "1.2345" is refused, not read as a number nobody typed).
// Anything else -- letters, a repeated decimal separator, a third decimal
// digit -- is `null`, and the form says so rather than storing a guess.

/** The decimal separators a user may type. */
const SEPARATORS = [".", ","] as const;

/** Every space a pasted number can carry. `\s` already covers the non-breaking
 *  and thin spaces a spreadsheet groups with; the zero-width space does not
 *  count as whitespace in JavaScript and has to be named. */
const SPACES = /[\s\u200b]/g;

/** The integer part with no grouping at all. */
const PLAIN_DIGITS = /^[0-9]*$/;

/** The integer part grouped in threes under one consistent separator. */
const GROUPED_DIGITS = /^(?:[0-9]{1,3}(?:\.[0-9]{3})+|[0-9]{1,3}(?:,[0-9]{3})+)$/;

/**
 * Parses a typed decimal into whole hundredths -- cents for an amount, basis
 * points for a percentage -- or `null` when the text is not a number.
 *
 * Never rounds: a third decimal is a refusal, not a silent truncation, because
 * a price the user did not type is worse than a form that asks again.
 */
export function parseHundredths(text: string): number | null {
  const compact = text.replace(SPACES, "");
  if (compact === "") return null;

  const sign = compact.startsWith("-") ? -1 : 1;
  const body = compact.replace(/^[+-]/, "");
  if (!/^[0-9.,]*$/.test(body) || !/[0-9]/.test(body)) return null;

  const decimal = decimalSeparator(body);
  const parts = decimal === null ? [body] : body.split(decimal);
  if (parts.length > 2) return null; // the same separator twice: not a number
  const integerPart = parts[0] ?? "";
  const fraction = parts[1] ?? "";
  if (!/^[0-9]{0,2}$/.test(fraction)) return null;
  if (!PLAIN_DIGITS.test(integerPart) && !GROUPED_DIGITS.test(integerPart)) return null;

  const whole = SEPARATORS.reduce((acc, s) => acc.split(s).join(""), integerPart);
  if (whole === "" && fraction === "") return null;

  // Assembled from the two integer halves rather than parsed as a float: a
  // price must never depend on whether `1.15 * 100` lands on 115 or 114.999.
  const hundredths = (whole === "" ? 0 : Number(whole)) * 100 + Number(fraction.padEnd(2, "0"));
  return Number.isSafeInteger(hundredths) ? sign * hundredths : null;
}

/** Which of `.` / `,` in `body` is the decimal separator, if either is. */
function decimalSeparator(body: string): string | null {
  const present = SEPARATORS.filter((s) => body.includes(s));
  if (present.length === 2) {
    // Mixed notation ("1.234,56" / "1,234.56"): the last one decides.
    const [first, second] = present;
    if (first === undefined || second === undefined) return null;
    return body.lastIndexOf(first) > body.lastIndexOf(second) ? first : second;
  }
  const only = present[0];
  if (only === undefined) return null;
  if (body.indexOf(only) !== body.lastIndexOf(only)) return null; // repeated: grouping
  const after = body.length - body.indexOf(only) - 1;
  return after === 1 || after === 2 ? only : null;
}

/**
 * The editable form of whole hundredths: a plain number with a `.` decimal
 * separator and no grouping, so a prefilled field always parses back to
 * exactly the value it came from. Trailing zeros are dropped -- a 21 % rate
 * reads `21`, not `21.00`.
 */
export function hundredthsToInput(value: number): string {
  const sign = value < 0 ? "-" : "";
  const abs = Math.abs(value);
  const fraction = abs % 100;
  const whole = (abs - fraction) / 100;
  if (fraction === 0) return `${sign}${whole}`;
  const digits = fraction % 10 === 0 ? String(fraction / 10) : String(fraction).padStart(2, "0");
  return `${sign}${whole}.${digits}`;
}

/**
 * An amount for reading: grouped and always two decimals, in `locale`'s
 * convention. `currency` renders the symbol; omit it for the price list, which
 * is quoted in the tenant's own currency and carries no per-row currency.
 */
export function formatAmount(cents: number, locale: string, currency?: string): string {
  const options: Intl.NumberFormatOptions =
    currency === undefined
      ? { minimumFractionDigits: 2, maximumFractionDigits: 2 }
      : { style: "currency", currency, minimumFractionDigits: 2, maximumFractionDigits: 2 };
  try {
    return new Intl.NumberFormat(locale, options).format(cents / 100);
  } catch {
    // An unknown currency code (the server validates shape, not the ISO list)
    // must not blank a price list.
    return hundredthsToInput(cents);
  }
}

/** A VAT rate for reading: basis points as a percentage ("2100" -> "21%"), with
 *  the spacing each language puts before its percent sign. */
export function formatRate(basisPoints: number, locale: string): string {
  return new Intl.NumberFormat(locale, {
    style: "percent",
    maximumFractionDigits: 2,
  }).format(basisPoints / 10000);
}
