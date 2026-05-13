"use client";

import { useMemo, useState } from "react";
import {
  FAQ_SUPPORTED_LOCALES,
  type FaqCatalog,
  type FaqItem,
  type FaqLocale,
  type FaqSection,
  type FaqLocalizedText,
} from "@prophetica/ts-sdk/support-faq";
import { cn } from "@prophetica/ui";

interface FaqManagementFormProps {
  initialCatalog: FaqCatalog;
  initialSource: "managed" | "default";
  canEdit: boolean;
}

type SaveState = "idle" | "saving" | "saved" | "error";

const localeLabels: Record<FaqLocale, string> = {
  "zh-Hant": "繁中",
  "zh-Hans": "簡中",
  en: "EN",
  id: "ID",
  vi: "VI",
  tl: "TL",
};

export function FaqManagementForm({
  initialCatalog,
  initialSource,
  canEdit,
}: FaqManagementFormProps) {
  const [catalog, setCatalog] = useState(initialCatalog);
  const [source, setSource] = useState(initialSource);
  const [locale, setLocale] = useState<FaqLocale>("zh-Hant");
  const [saveState, setSaveState] = useState<SaveState>("idle");
  const [message, setMessage] = useState("");

  const enabledItemCount = useMemo(
    () =>
      catalog.sections.reduce(
        (total, section) =>
          total + section.items.filter((item) => section.enabled && item.enabled).length,
        0,
      ),
    [catalog.sections],
  );

  async function saveCatalog() {
    if (!canEdit) return;
    setSaveState("saving");
    setMessage("");

    const response = await fetch("/api/faq/catalog", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ catalog }),
    });
    const body = (await response.json()) as {
      catalog?: FaqCatalog;
      source?: "managed" | "default";
      message?: string;
    };

    if (!response.ok || !body.catalog || !body.source) {
      setSaveState("error");
      setMessage(body.message ?? "FAQ 儲存失敗。");
      return;
    }

    setCatalog(body.catalog);
    setSource(body.source);
    setSaveState("saved");
    setMessage("FAQ 已發布。");
  }

  async function resetCatalog() {
    if (!canEdit || !window.confirm("確定要移除後台管理內容並回到系統預設 FAQ？")) {
      return;
    }
    setSaveState("saving");
    setMessage("");

    const response = await fetch("/api/faq/catalog", { method: "DELETE" });
    const body = (await response.json()) as {
      catalog?: FaqCatalog;
      source?: "managed" | "default";
      message?: string;
    };

    if (!response.ok || !body.catalog || !body.source) {
      setSaveState("error");
      setMessage(body.message ?? "FAQ 重置失敗。");
      return;
    }

    setCatalog(body.catalog);
    setSource(body.source);
    setSaveState("saved");
    setMessage("FAQ 已回到系統預設。");
  }

  function updateSection(sectionId: string, next: (section: FaqSection) => FaqSection) {
    setCatalog((current) => ({
      ...current,
      sections: current.sections.map((section) =>
        section.id === sectionId ? next(section) : section,
      ),
    }));
    setSaveState("idle");
  }

  function updateItem(
    sectionId: string,
    itemId: string,
    next: (item: FaqItem) => FaqItem,
  ) {
    updateSection(sectionId, (section) => ({
      ...section,
      items: section.items.map((item) => (item.id === itemId ? next(item) : item)),
    }));
  }

  function addSection() {
    const suffix = Date.now().toString(36);
    setCatalog((current) => ({
      ...current,
      sections: [
        ...current.sections,
        {
          id: `section_${suffix}`,
          order: nextOrder(current.sections),
          enabled: true,
          title: { [locale]: "新分類" },
          items: [],
        },
      ],
    }));
    setSaveState("idle");
  }

  function removeSection(sectionId: string) {
    setCatalog((current) => ({
      ...current,
      sections: current.sections.filter((section) => section.id !== sectionId),
    }));
    setSaveState("idle");
  }

  function addItem(sectionId: string) {
    const suffix = Date.now().toString(36);
    updateSection(sectionId, (section) => ({
      ...section,
      items: [
        ...section.items,
        {
          id: `item_${suffix}`,
          order: nextOrder(section.items),
          enabled: true,
          question: { [locale]: "新問題" },
          answer: { [locale]: "請填寫答案。" },
        },
      ],
    }));
  }

  function removeItem(sectionId: string, itemId: string) {
    updateSection(sectionId, (section) => ({
      ...section,
      items: section.items.filter((item) => item.id !== itemId),
    }));
  }

  return (
    <div className="space-y-6">
      <section className="grid gap-4 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5 md:grid-cols-[1fr_auto] md:items-center">
        <div>
          <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary">
            {source === "managed" ? "後台發布中" : "系統預設"}
          </p>
          <h2 className="mt-2 text-[21px] font-semibold leading-[1.19] text-proph-ink">
            {enabledItemCount} 則公開 FAQ
          </h2>
          <p className="mt-1 text-[14px] leading-[1.43] text-proph-ink-muted-80">
            最後更新：{catalog.updatedAt}，更新者：{catalog.updatedBy}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={saveCatalog}
            disabled={!canEdit || saveState === "saving"}
            className="inline-flex min-h-11 items-center rounded-proph-sm bg-proph-primary px-4 py-2 text-[14px] font-semibold text-proph-on-dark transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            {saveState === "saving" ? "發布中" : "發布 FAQ"}
          </button>
          <button
            type="button"
            onClick={resetCatalog}
            disabled={!canEdit || saveState === "saving"}
            className="inline-flex min-h-11 items-center rounded-proph-sm border border-proph-hairline bg-proph-canvas px-4 py-2 text-[14px] font-semibold text-proph-ink transition-colors hover:bg-proph-divider-soft disabled:opacity-50"
          >
            回到預設
          </button>
        </div>
      </section>

      {!canEdit && (
        <p className="rounded-proph-lg border border-proph-hairline bg-proph-canvas-parchment px-4 py-3 text-[14px] text-proph-ink-muted-80">
          你的角色可以檢視 FAQ，但只有 admin 能發布或重置內容。
        </p>
      )}

      {message && (
        <p
          className={cn(
            "rounded-proph-lg border px-4 py-3 text-[14px]",
            saveState === "error"
              ? "border-proph-feedback-danger/40 bg-proph-feedback-danger/10 text-proph-ink"
              : "border-proph-feedback-success/40 bg-proph-feedback-success/10 text-proph-ink",
          )}
        >
          {message}
        </p>
      )}

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap gap-2" aria-label="FAQ 語系">
            {FAQ_SUPPORTED_LOCALES.map((candidate) => (
              <button
                type="button"
                key={candidate}
                onClick={() => setLocale(candidate)}
                className={cn(
                  "inline-flex min-h-10 items-center rounded-proph-pill border px-4 py-1.5 text-[14px] font-semibold",
                  locale === candidate
                    ? "border-proph-primary bg-proph-primary text-proph-on-dark"
                    : "border-proph-hairline bg-proph-canvas text-proph-ink-muted-80 hover:text-proph-ink",
                )}
              >
                {localeLabels[candidate]}
              </button>
            ))}
          </div>
          <button
            type="button"
            onClick={addSection}
            disabled={!canEdit}
            className="inline-flex min-h-10 items-center rounded-proph-sm border border-proph-hairline bg-proph-canvas px-4 py-2 text-[14px] font-semibold text-proph-ink hover:bg-proph-divider-soft disabled:opacity-50"
          >
            新增分類
          </button>
        </div>
      </section>

      <div className="space-y-5">
        {catalog.sections.map((section) => (
          <section
            key={section.id}
            className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
          >
            <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto]">
              <div className="space-y-2">
                <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
                  {section.id}
                </p>
                <TextField
                  label="分類標題"
                  value={section.title[locale] ?? ""}
                  disabled={!canEdit}
                  onChange={(value) =>
                    updateSection(section.id, (current) => ({
                      ...current,
                      title: withLocalizedText(current.title, locale, value),
                    }))
                  }
                />
              </div>
              <div className="flex flex-wrap items-start gap-2">
                <label className="inline-flex min-h-10 items-center gap-2 rounded-proph-sm border border-proph-hairline px-3 text-[14px]">
                  <input
                    type="checkbox"
                    checked={section.enabled}
                    disabled={!canEdit}
                    onChange={(event) =>
                      updateSection(section.id, (current) => ({
                        ...current,
                        enabled: event.target.checked,
                      }))
                    }
                  />
                  公開
                </label>
                <button
                  type="button"
                  onClick={() => addItem(section.id)}
                  disabled={!canEdit}
                  className="inline-flex min-h-10 items-center rounded-proph-sm border border-proph-hairline px-3 text-[14px] font-semibold hover:bg-proph-divider-soft disabled:opacity-50"
                >
                  新增問題
                </button>
                <button
                  type="button"
                  onClick={() => removeSection(section.id)}
                  disabled={!canEdit}
                  className="inline-flex min-h-10 items-center rounded-proph-sm border border-proph-feedback-danger/40 px-3 text-[14px] font-semibold text-proph-feedback-danger hover:bg-proph-feedback-danger/10 disabled:opacity-50"
                >
                  刪除分類
                </button>
              </div>
            </div>

            <div className="mt-5 space-y-4">
              {section.items.map((item) => (
                <article
                  key={item.id}
                  className="rounded-proph-lg border border-proph-divider-soft bg-proph-canvas-parchment p-4"
                >
                  <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                    <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
                      {item.id}
                    </p>
                    <div className="flex flex-wrap gap-2">
                      <label className="inline-flex min-h-10 items-center gap-2 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 text-[14px]">
                        <input
                          type="checkbox"
                          checked={item.enabled}
                          disabled={!canEdit}
                          onChange={(event) =>
                            updateItem(section.id, item.id, (current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))
                          }
                        />
                        公開
                      </label>
                      <button
                        type="button"
                        onClick={() => removeItem(section.id, item.id)}
                        disabled={!canEdit}
                        className="inline-flex min-h-10 items-center rounded-proph-sm border border-proph-feedback-danger/40 bg-proph-canvas px-3 text-[14px] font-semibold text-proph-feedback-danger hover:bg-proph-feedback-danger/10 disabled:opacity-50"
                      >
                        刪除
                      </button>
                    </div>
                  </div>
                  <div className="grid gap-4">
                    <TextField
                      label="問題"
                      value={item.question[locale] ?? ""}
                      disabled={!canEdit}
                      onChange={(value) =>
                        updateItem(section.id, item.id, (current) => ({
                          ...current,
                          question: withLocalizedText(current.question, locale, value),
                        }))
                      }
                    />
                    <TextArea
                      label="答案"
                      value={item.answer[locale] ?? ""}
                      disabled={!canEdit}
                      onChange={(value) =>
                        updateItem(section.id, item.id, (current) => ({
                          ...current,
                          answer: withLocalizedText(current.answer, locale, value),
                        }))
                      }
                    />
                  </div>
                </article>
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}

function TextField({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string;
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block">
      <span className="text-[13px] font-semibold text-proph-ink-muted-80">{label}</span>
      <input
        type="text"
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        className="mt-1 min-h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 text-[15px] text-proph-ink outline-none transition-colors focus:border-proph-primary disabled:opacity-70"
      />
    </label>
  );
}

function TextArea({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string;
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block">
      <span className="text-[13px] font-semibold text-proph-ink-muted-80">{label}</span>
      <textarea
        value={value}
        disabled={disabled}
        rows={4}
        onChange={(event) => onChange(event.target.value)}
        className="mt-1 w-full resize-y rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 text-[15px] leading-[1.5] text-proph-ink outline-none transition-colors focus:border-proph-primary disabled:opacity-70"
      />
    </label>
  );
}

function withLocalizedText(
  current: FaqLocalizedText,
  locale: FaqLocale,
  value: string,
): FaqLocalizedText {
  return {
    ...current,
    [locale]: value,
  };
}

function nextOrder(items: Array<{ order: number }>): number {
  return Math.max(0, ...items.map((item) => item.order)) + 10;
}
