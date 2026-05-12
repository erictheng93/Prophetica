"use client";

import { useMemo, useState, type FormEvent } from "react";
import type { Role } from "@/lib/rbac";
import {
  formatMinorUnitsAsPoints,
  parsePointAmountToMinorUnits,
  type ManualPointDirection,
  type ManualPointsAdjustmentResponse,
  type WalletBalance,
} from "@/lib/points";

interface PointsOperationsProps {
  viewerRole: Role;
}

const directionLabels: Record<ManualPointDirection, string> = {
  grant: "增加點數",
  deduct: "扣除點數",
};

export function PointsOperations({ viewerRole }: PointsOperationsProps) {
  const [lookupUserId, setLookupUserId] = useState("");
  const [balance, setBalance] = useState<WalletBalance | null>(null);
  const [balanceError, setBalanceError] = useState<string | null>(null);
  const [balanceLoading, setBalanceLoading] = useState(false);

  const [targetUserId, setTargetUserId] = useState("");
  const [direction, setDirection] = useState<ManualPointDirection>("grant");
  const [amount, setAmount] = useState("");
  const [reasonCode, setReasonCode] = useState("ops_adjustment");
  const [reasonText, setReasonText] = useState("");
  const [adjustmentError, setAdjustmentError] = useState<string | null>(null);
  const [adjustmentResult, setAdjustmentResult] =
    useState<ManualPointsAdjustmentResponse | null>(null);
  const [adjustmentLoading, setAdjustmentLoading] = useState(false);

  const canSubmit =
    viewerRole === "admin" ||
    viewerRole === "manager" ||
    viewerRole === "affiliate";
  const allowedDirections: ManualPointDirection[] =
    viewerRole === "affiliate" ? ["grant"] : ["grant", "deduct"];
  const idempotencyPreview = useMemo(
    () => `manual-points:${direction}:<uuid-v4>`,
    [direction],
  );

  async function lookupBalance(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const userId = lookupUserId.trim();
    if (!userId) {
      setBalanceError("請輸入會員 ID。");
      return;
    }

    setBalanceLoading(true);
    setBalanceError(null);
    setBalance(null);
    try {
      const response = await fetch(
        `/api/points/wallets/${encodeURIComponent(userId)}/balance`,
        {
          cache: "no-store",
        },
      );
      if (!response.ok) {
        setBalanceError(`查詢失敗：${await errorText(response)}`);
        return;
      }
      setBalance((await response.json()) as WalletBalance);
    } catch {
      setBalanceError("查詢失敗：無法連線至後端。");
    } finally {
      setBalanceLoading(false);
    }
  }

  async function submitAdjustment(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAdjustmentError(null);
    setAdjustmentResult(null);

    let deltaMinorUnits: number;
    try {
      deltaMinorUnits = parsePointAmountToMinorUnits(amount);
    } catch (error) {
      setAdjustmentError(
        error instanceof Error ? error.message : "點數格式不正確。",
      );
      return;
    }

    if (!targetUserId.trim()) {
      setAdjustmentError("請輸入目標會員 ID。");
      return;
    }
    if (!reasonCode.trim() || !reasonText.trim()) {
      setAdjustmentError("請填寫 reason code 與原因說明。");
      return;
    }
    if (!allowedDirections.includes(direction)) {
      setAdjustmentError("此角色不允許扣除點數。");
      return;
    }

    const idempotencyKey = `manual-points:${direction}:${crypto.randomUUID()}`;
    setAdjustmentLoading(true);
    try {
      const response = await fetch("/api/points/manual-adjustments", {
        method: "POST",
        cache: "no-store",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          target_user_id: targetUserId.trim(),
          direction,
          delta_minor_units: deltaMinorUnits,
          reason_code: reasonCode.trim(),
          reason_text: reasonText.trim(),
          idempotency_key: idempotencyKey,
        }),
      });
      if (!response.ok) {
        setAdjustmentError(`提交失敗：${await errorText(response)}`);
        return;
      }
      setAdjustmentResult(
        (await response.json()) as ManualPointsAdjustmentResponse,
      );
      setLookupUserId(targetUserId.trim());
    } catch {
      setAdjustmentError("提交失敗：無法連線至後端。");
    } finally {
      setAdjustmentLoading(false);
    }
  }

  return (
    <div className="grid gap-6 xl:grid-cols-[0.9fr_1.1fr]">
      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          會員點數查詢
        </h2>
        <form className="mt-5 space-y-4" onSubmit={lookupBalance}>
          <label className="block">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              會員 ID
            </span>
            <input
              value={lookupUserId}
              onChange={(event) => setLookupUserId(event.target.value)}
              className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus"
              placeholder="user_123"
            />
          </label>
          <button
            type="submit"
            disabled={balanceLoading}
            className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            {balanceLoading ? "查詢中" : "查詢餘額"}
          </button>
        </form>

        {balanceError && (
          <p className="mt-4 rounded-proph-sm border border-proph-feedback-error bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
            {balanceError}
          </p>
        )}

        {balance && (
          <dl className="mt-5 grid gap-3">
            <BalanceRow
              label="可用點數"
              value={formatMinorUnitsAsPoints(balance.available_minor_units, {
                liabilityBalance: true,
              })}
            />
            <BalanceRow
              label="鎖倉點數"
              value={formatMinorUnitsAsPoints(balance.locked_minor_units, {
                liabilityBalance: true,
              })}
            />
            <BalanceRow
              label="凍結點數"
              value={formatMinorUnitsAsPoints(balance.frozen_minor_units, {
                liabilityBalance: true,
              })}
            />
          </dl>
        )}
      </section>

      <section className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
        <h2 className="text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          手動點數調整
        </h2>
        <form
          className="mt-5 grid gap-4 md:grid-cols-2"
          onSubmit={submitAdjustment}
        >
          <label className="block">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              目標會員 ID
            </span>
            <input
              value={targetUserId}
              onChange={(event) => setTargetUserId(event.target.value)}
              disabled={!canSubmit || adjustmentLoading}
              className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
              placeholder="user_123"
            />
          </label>

          <label className="block">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              方向
            </span>
            <select
              value={direction}
              onChange={(event) =>
                setDirection(event.target.value as ManualPointDirection)
              }
              disabled={!canSubmit || adjustmentLoading}
              className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
            >
              {allowedDirections.map((option) => (
                <option key={option} value={option}>
                  {directionLabels[option]}
                </option>
              ))}
            </select>
          </label>

          <label className="block">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              點數
            </span>
            <input
              value={amount}
              onChange={(event) => setAmount(event.target.value)}
              disabled={!canSubmit || adjustmentLoading}
              inputMode="decimal"
              className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
              placeholder="100.00"
            />
          </label>

          <label className="block">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              Reason code
            </span>
            <input
              value={reasonCode}
              onChange={(event) => setReasonCode(event.target.value)}
              disabled={!canSubmit || adjustmentLoading}
              className="mt-2 h-11 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 text-[14px] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
            />
          </label>

          <label className="block md:col-span-2">
            <span className="text-[13px] font-semibold text-proph-ink-muted-80">
              原因說明
            </span>
            <textarea
              value={reasonText}
              onChange={(event) => setReasonText(event.target.value)}
              disabled={!canSubmit || adjustmentLoading}
              className="mt-2 min-h-24 w-full rounded-proph-sm border border-proph-hairline bg-proph-surface-pearl px-3 py-2 text-[14px] leading-[1.43] tracking-[-0.224px] outline-none focus:border-proph-primary-focus disabled:opacity-60"
              placeholder="輸入調整依據、工單或活動原因"
            />
          </label>

          <div className="md:col-span-2">
            <button
              type="submit"
              disabled={!canSubmit || adjustmentLoading}
              className="inline-flex h-11 items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-50"
            >
              {adjustmentLoading ? "提交中" : "提交調整"}
            </button>
            <p className="mt-3 font-mono text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-48">
              {idempotencyPreview}
            </p>
          </div>
        </form>

        {adjustmentError && (
          <p className="mt-4 rounded-proph-sm border border-proph-feedback-error bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
            {adjustmentError}
          </p>
        )}

        {adjustmentResult && (
          <div className="mt-4 rounded-proph-sm border border-proph-feedback-success bg-proph-surface-pearl p-3 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
            <p>已建立 ledger transaction #{adjustmentResult.txn_id}</p>
            <p className="mt-1 font-mono text-[12px] text-proph-ink-muted-80">
              audit_log_id: {adjustmentResult.audit_log_id ?? "—"}
            </p>
          </div>
        )}
      </section>
    </div>
  );
}

function BalanceRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-proph-sm border border-proph-divider-soft bg-proph-surface-pearl p-3">
      <dt className="text-[13px] font-semibold text-proph-ink-muted-80">
        {label}
      </dt>
      <dd className="font-mono text-[15px] tracking-[-0.224px] text-proph-ink">
        {value}
      </dd>
    </div>
  );
}

async function errorText(response: Response): Promise<string> {
  try {
    const payload = (await response.json()) as {
      error?: string;
      message?: string;
    };
    return payload.message ?? payload.error ?? String(response.status);
  } catch {
    return String(response.status);
  }
}
