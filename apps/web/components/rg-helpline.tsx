import { PhoneCall } from "@phosphor-icons/react/dist/ssr";

// SPEC-006 §7.1 — Mandatory help-line strip on responsible-gaming surfaces.
// Numbers below are well-established Taiwan public hotlines. Final approved
// list (incl. licensing-region resources for Malta / Curaçao / etc.) is
// pending Compliance Officer sign-off; update this file as the source of truth.

interface Helpline {
  label: string;
  number: string;
  hours: string;
}

const helplines: Helpline[] = [
  { label: "衛福部安心專線", number: "1925", hours: "24 小時" },
  { label: "毒品危害防制中心", number: "0800-770-885", hours: "24 小時" },
  { label: "張老師專線", number: "1980", hours: "週一至週六" },
];

export function RGHelpline() {
  return (
    <section
      className="w-full bg-proph-surface-tile-1 text-proph-on-dark"
      aria-label="責任博弈援助資源"
    >
      <div className="mx-auto max-w-proph px-6 py-10 md:px-10">
        <div className="mb-6 flex flex-col gap-1">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-body-muted">
            需要協助
          </span>
          <h3 className="font-display text-[28px] font-normal leading-[1.14] tracking-[0.196px]">
            如果你或身邊的人對遊戲行為感到困擾，這些專線 24 小時為你而開。
          </h3>
        </div>

        <ul className="grid grid-cols-1 gap-4 md:grid-cols-3">
          {helplines.map((line) => (
            <li
              key={line.number}
              className="rounded-proph-lg border border-proph-on-dark/15 px-5 py-4"
            >
              <div className="font-text text-[12px] leading-none tracking-[-0.12px] text-proph-body-muted">
                {line.label} · {line.hours}
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
          如有立即危險請撥打 119 或 110。撥打前述專線完全免費，且不會留下紀錄於你的 prophetica
          帳號。
        </p>
      </div>
    </section>
  );
}
