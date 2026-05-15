import {
  archiveAffiliateAction,
  createAffiliateAction,
  updateAffiliateAction,
} from "@/app/affiliate/actions";
import type { AffiliateDetail } from "@/lib/affiliates";
import type { Role } from "@/lib/rbac";

const inputClass =
  "min-h-11 rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 text-[14px] tracking-[-0.224px] text-proph-ink";
const labelClass =
  "flex flex-col gap-1 text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48";
const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50";

export function AffiliateCreateForm({ role }: { role: Role }) {
  const disabled = role !== "admin" && role !== "manager";
  const isAdmin = role === "admin";
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5 md:p-6">
      <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        新增代理
      </h2>
      <form action={createAffiliateAction} className="mt-4 grid gap-3 md:grid-cols-4">
        <Field name="id" label="代理 ID" placeholder="AFF-200" required />
        <Field name="userId" label="User ID" placeholder="user_aff_200" required />
        <Field name="displayName" label="名稱" placeholder="Orion Partners" required />
        <Field name="emailMasked" label="脫敏 Email" placeholder="o***@example.com" required />
        <Select name="status" label="狀態" defaultValue="onboarding" options={affiliateStatusOptions} />
        <NumberField name="commissionRateBps" label="佣金 BPS" defaultValue={600} />
        <Select name="tier" label="層級" defaultValue="1" options={tierOptions} />
        <Select
          name="creditRating"
          label="信用評級"
          defaultValue="restricted"
          options={creditRatingOptions}
          disabled={!isAdmin}
        />
        <Select
          name="creditStatus"
          label="信用狀態"
          defaultValue="pending_review"
          options={creditStatusOptions}
          disabled={!isAdmin}
        />
        <NumberField name="maxActiveCreditAccounts" label="Active 上限" defaultValue={0} disabled={!isAdmin} />
        <NumberField name="totalCreditLimitMinorUnits" label="總授信 minor" defaultValue={0} disabled={!isAdmin} />
        <NumberField name="perAccountLimitMinorUnits" label="單戶 minor" defaultValue={0} disabled={!isAdmin} />
        <Field name="creditReviewExpiresAt" label="覆核到期" type="date" disabled={!isAdmin} />
        <Field name="reason" label="原因" required className="md:col-span-2" />
        <Field name="authorizationBasis" label="授權依據" required className="md:col-span-2" />
        <div className="md:col-span-4">
          <button type="submit" className={buttonClass} disabled={disabled}>
            建立代理
          </button>
        </div>
      </form>
    </section>
  );
}

export function AffiliateUpdateForm({
  affiliate,
  role,
}: {
  affiliate: AffiliateDetail;
  role: Role;
}) {
  const isAdmin = role === "admin";
  return (
    <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
      <h3 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
        編輯代理資料
      </h3>
      <form action={updateAffiliateAction} className="mt-4 grid gap-3 md:grid-cols-4">
        <input type="hidden" name="affiliateId" value={affiliate.id} />
        <Field name="displayName" label="名稱" defaultValue={affiliate.displayName} />
        <Field name="emailMasked" label="脫敏 Email" defaultValue={affiliate.emailMasked} />
        <Select name="status" label="狀態" defaultValue={affiliate.status} options={affiliateStatusOptions} />
        <NumberField name="commissionRateBps" label="佣金 BPS" defaultValue={affiliate.commissionRateBps} />
        <Select name="tier" label="層級" defaultValue={String(affiliate.tier)} options={tierOptions} />
        <Field name="ownerSponsorId" label="上層 Sponsor" defaultValue={affiliate.ownerSponsorId ?? ""} />
        <Select
          name="creditRating"
          label="信用評級"
          defaultValue={affiliate.creditProfile.rating}
          options={creditRatingOptions}
          disabled={!isAdmin}
        />
        <Select
          name="creditStatus"
          label="信用狀態"
          defaultValue={affiliate.creditProfile.status}
          options={creditStatusOptions}
          disabled={!isAdmin}
        />
        <NumberField
          name="maxActiveCreditAccounts"
          label="Active 上限"
          defaultValue={affiliate.creditProfile.maxActiveCreditAccounts}
          disabled={!isAdmin}
        />
        <NumberField
          name="totalCreditLimitMinorUnits"
          label="總授信 minor"
          defaultValue={affiliate.creditProfile.totalCreditLimitMinorUnits}
          disabled={!isAdmin}
        />
        <NumberField
          name="perAccountLimitMinorUnits"
          label="單戶 minor"
          defaultValue={affiliate.creditProfile.perAccountLimitMinorUnits}
          disabled={!isAdmin}
        />
        <Field
          name="creditReviewExpiresAt"
          label="覆核到期"
          type="date"
          defaultValue={affiliate.creditProfile.reviewExpiresAt ?? ""}
          disabled={!isAdmin}
        />
        <Field name="effectiveAt" label="生效日" type="date" />
        <Field name="reason" label="原因" required className="md:col-span-2" />
        <Field name="authorizationBasis" label="授權依據" required className="md:col-span-2" />
        <div className="md:col-span-4">
          <button type="submit" className={buttonClass}>
            儲存變更
          </button>
        </div>
      </form>
      {!isAdmin && (
        <p className="mt-3 text-[12px] leading-[1.33] tracking-[-0.12px] text-proph-ink-muted-80">
          Manager 可更新基本代理資料；信用評級與授信欄位需 admin 執行。
        </p>
      )}
    </section>
  );
}

export function AffiliateArchiveForm({ affiliate }: { affiliate: AffiliateDetail }) {
  return (
    <form action={archiveAffiliateAction} className="mt-4 grid gap-3 md:grid-cols-3">
      <input type="hidden" name="affiliateId" value={affiliate.id} />
      <Field name="reason" label="封存原因" required />
      <Field name="authorizationBasis" label="授權依據" required />
      <div className="flex items-end">
        <button
          type="submit"
          className="inline-flex min-h-11 items-center justify-center rounded-proph-pill border border-proph-feedback-error px-5 text-[14px] font-semibold text-proph-feedback-error disabled:cursor-not-allowed disabled:opacity-50"
          disabled={affiliate.status === "archived"}
        >
          封存代理
        </button>
      </div>
    </form>
  );
}

const affiliateStatusOptions = [
  ["onboarding", "註冊中"],
  ["active", "啟用"],
  ["review", "審查中"],
  ["frozen", "凍結"],
  ["archived", "封存"],
] as const;

const creditRatingOptions = [
  ["A", "A"],
  ["B", "B"],
  ["C", "C"],
  ["restricted", "受限"],
  ["suspended", "暫停"],
] as const;

const creditStatusOptions = [
  ["pending_review", "待審"],
  ["active", "啟用"],
  ["limited", "受限"],
  ["frozen", "凍結"],
  ["closed", "關閉"],
] as const;

const tierOptions = [
  ["1", "Tier 1"],
  ["2", "Tier 2"],
  ["3", "Tier 3"],
] as const;

function Field({
  name,
  label,
  defaultValue,
  placeholder,
  required,
  type = "text",
  className = "",
  disabled,
}: {
  name: string;
  label: string;
  defaultValue?: string;
  placeholder?: string;
  required?: boolean;
  type?: string;
  className?: string;
  disabled?: boolean;
}) {
  return (
    <label className={`${labelClass} ${className}`}>
      {label}
      <input
        className={inputClass}
        name={name}
        type={type}
        defaultValue={defaultValue}
        placeholder={placeholder}
        required={required}
        disabled={disabled}
      />
    </label>
  );
}

function NumberField({
  name,
  label,
  defaultValue,
  disabled,
}: {
  name: string;
  label: string;
  defaultValue: number;
  disabled?: boolean;
}) {
  return (
    <label className={labelClass}>
      {label}
      <input
        className={inputClass}
        name={name}
        type="number"
        min={0}
        defaultValue={defaultValue}
        disabled={disabled}
      />
    </label>
  );
}

function Select({
  name,
  label,
  defaultValue,
  options,
  disabled,
}: {
  name: string;
  label: string;
  defaultValue: string;
  options: readonly (readonly [string, string])[];
  disabled?: boolean;
}) {
  return (
    <label className={labelClass}>
      {label}
      <select className={inputClass} name={name} defaultValue={defaultValue} disabled={disabled}>
        {options.map(([value, text]) => (
          <option key={value} value={value}>
            {text}
          </option>
        ))}
      </select>
    </label>
  );
}
