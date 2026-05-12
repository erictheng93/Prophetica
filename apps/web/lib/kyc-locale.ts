// SPEC-005a §2 — KycSessionStartParams.locale must be one of the SumSub-
// recognised BCP-47 codes (zh-TW / zh-CN / ja-JP / ko-KR / vi-VN / ms-MY /
// en). next-intl exposes app locales like `zh-Hant` and `zh-Hans` (script
// subtags) that would otherwise reach SumSub as unknown codes. This module is
// the only place that knows how to translate.
//
// Keep this list in lockstep with messages/<locale>.json files: when a new
// locale is added in i18n/request.ts, add the corresponding mapping here.

export type SumsubLocale =
  | "zh-TW"
  | "zh-CN"
  | "ja-JP"
  | "ko-KR"
  | "vi-VN"
  | "ms-MY"
  | "en";

export const SUMSUB_DEFAULT_LOCALE: SumsubLocale = "en";

export function mapLocaleToSumsub(locale: string): SumsubLocale {
  switch (locale) {
    case "zh-Hant":
    case "zh-TW":
      return "zh-TW";
    case "zh-Hans":
    case "zh-CN":
      return "zh-CN";
    case "ja":
    case "ja-JP":
      return "ja-JP";
    case "ko":
    case "ko-KR":
      return "ko-KR";
    case "vi":
    case "vi-VN":
      return "vi-VN";
    case "ms":
    case "ms-MY":
      return "ms-MY";
    case "id":
    case "id-ID":
    case "tl":
    case "fil":
      return "en";
    case "en":
    case "en-US":
    case "en-GB":
      return "en";
    default:
      // Unknown locale — fall back to English rather than passing through an
      // unrecognised code that SumSub would silently reject.
      return SUMSUB_DEFAULT_LOCALE;
  }
}
