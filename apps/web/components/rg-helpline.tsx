import { PhoneCall } from "@phosphor-icons/react/dist/ssr";
import { getTranslations } from "next-intl/server";

// SPEC-006 §7.1 — Mandatory help-line strip on responsible-gaming surfaces.
// Numbers stay literal (TW-specific public hotlines verified against published
// sources); labels / hours / emergency note translate. Final approved list
// (incl. licensing-region resources for Malta / Curaçao / etc.) is pending
// Compliance Officer sign-off — extend HELPLINE_DATA + add catalogue entries
// when that lands.

interface HelplineData {
  /** i18n key under `responsibleGaming.helpline.items` */
  key: string;
  /** Phone number — not translated. */
  number: string;
}

const HELPLINE_DATA: HelplineData[] = [
  { key: "mhwSafe", number: "1925" },
  { key: "drugPrevention", number: "0800-770-885" },
  { key: "teacherZhang", number: "1980" },
];

export async function RGHelpline() {
  const t = await getTranslations("responsibleGaming.helpline");
  return (
    <section
      className="w-full bg-proph-surface-tile-1 text-proph-on-dark"
      aria-label={t("ariaLabel")}
    >
      <div className="mx-auto max-w-proph px-6 py-10 md:px-10">
        <div className="mb-6 flex flex-col gap-1">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-body-muted">
            {t("kicker")}
          </span>
          <h3 className="font-display text-[28px] font-normal leading-[1.14] tracking-[0.196px]">
            {t("headline")}
          </h3>
        </div>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-3">
          {HELPLINE_DATA.map((line) => (
            <li
              key={line.number}
              className="rounded-proph-lg border border-proph-on-dark/15 px-5 py-4"
            >
              <div className="font-text text-[12px] leading-none tracking-[-0.12px] text-proph-body-muted">
                {t(`items.${line.key}.label`)} · {t(`items.${line.key}.hours`)}
              </div>
              <a
                href={`tel:${line.number.replace(/-/g, "")}`}
                className="mt-2 inline-flex items-center gap-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px] text-proph-primary-on-dark"
              >
                <PhoneCall weight="bold" className="h-7 w-7" aria-hidden="true" />
                {line.number}
              </a>
            </li>
          ))}
        </ul>

        <p className="mt-6 font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-body-muted">
          {t("emergencyNote")}
        </p>
      </div>
    </section>
  );
}
