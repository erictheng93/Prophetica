import { AdminShell } from "@/components/admin-shell";
import { RoleGate } from "@/components/role-gate";
import type { ReactNode } from "react";
import { fetchAdminAffiliateList } from "@/lib/affiliate-management-bff";
import { fetchManagerAccounts } from "@/lib/manager-accounts-bff";
import {
  createManagerAccountAction,
  disableManagerAccountAction,
  reassignAffiliateManagerAction,
  updateManagerAccountAction,
} from "./actions";

interface ManagerAccountsPageProps {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}

export default function ManagerAccountsPage({
  searchParams,
}: ManagerAccountsPageProps) {
  return (
    <RoleGate allow={["admin"]}>
      {async (viewer) => {
        const resolvedSearchParams: Promise<
          Record<string, string | string[] | undefined>
        > = searchParams ?? Promise.resolve({});
        const [managers, affiliates, params] = await Promise.all([
          fetchManagerAccounts(viewer),
          fetchAdminAffiliateList(viewer),
          resolvedSearchParams,
        ]);
        const status = firstParam(params.status);
        const error = firstParam(params.error);
        const activeManagers = managers.filter((manager) => !manager.disabled);
        return (
          <AdminShell
            eyebrow="帳號設定"
            title="Manager 帳號"
            description="管理可登入後台並負責代理範圍的 manager 帳號。"
          >
            {(status || error) && (
              <div
                className={[
                  "mb-4 border px-4 py-3 text-[14px]",
                  error
                    ? "border-red-200 bg-red-50 text-red-800"
                    : "border-emerald-200 bg-emerald-50 text-emerald-800",
                ].join(" ")}
              >
                {messageForStatus(status, error)}
              </div>
            )}

            <section className="mb-6 border border-proph-hairline bg-proph-canvas p-5 md:p-6">
              <h2 className="mb-4 text-[16px] font-semibold">新增 manager</h2>
              <form
                action={createManagerAccountAction}
                className="grid gap-3 md:grid-cols-2 xl:grid-cols-4"
              >
                <Input name="userId" label="User ID" required />
                <Input name="displayName" label="Display name" required />
                <Input name="email" label="Email" type="email" required />
                <Input name="phone" label="Phone" required />
                <Input name="password" label="Password" type="password" required />
                <Input name="country" label="Country" defaultValue="TW" />
                <Input name="timezone" label="Timezone" defaultValue="Asia/Taipei" />
                <Input name="tier" label="Tier" defaultValue="standard" />
                <Input name="reason" label="Reason" required />
                <Input
                  name="authorizationBasis"
                  label="Authorization basis"
                  required
                />
                <div className="flex items-end">
                  <button className="h-10 bg-proph-ink px-4 text-[14px] font-semibold text-white">
                    新增
                  </button>
                </div>
              </form>
            </section>

            <section className="mb-6 border border-proph-hairline bg-proph-canvas p-5 md:p-6">
              <h2 className="mb-4 text-[16px] font-semibold">代理重指派</h2>
              <form
                action={reassignAffiliateManagerAction}
                className="grid gap-3 md:grid-cols-2 xl:grid-cols-5"
              >
                <Select name="affiliateId" label="Affiliate" required>
                  {affiliates
                    .filter((affiliate) => affiliate.status !== "archived")
                    .map((affiliate) => (
                      <option key={affiliate.id} value={affiliate.id}>
                        {affiliate.id} · {affiliate.displayName}
                      </option>
                    ))}
                </Select>
                <Select name="managingManagerId" label="Target manager" required>
                  {activeManagers.map((manager) => (
                    <option key={manager.user_id} value={manager.user_id}>
                      {manager.user_id}
                    </option>
                  ))}
                </Select>
                <Input name="reason" label="Reason" required />
                <Input
                  name="authorizationBasis"
                  label="Authorization basis"
                  required
                />
                <div className="flex items-end">
                  <button className="h-10 bg-proph-ink px-4 text-[14px] font-semibold text-white">
                    指派
                  </button>
                </div>
              </form>
            </section>

            <section className="border border-proph-hairline bg-proph-canvas p-5 md:p-6">
              <div className="overflow-x-auto">
                <table className="min-w-full text-left text-[14px]">
                  <thead className="text-[12px] uppercase tracking-[0.08em] text-proph-ink-muted-48">
                    <tr>
                      <th className="px-3 py-2 font-semibold">User ID</th>
                      <th className="px-3 py-2 font-semibold">Email</th>
                      <th className="px-3 py-2 font-semibold">Phone</th>
                      <th className="px-3 py-2 font-semibold">Status</th>
                      <th className="px-3 py-2 font-semibold">Update</th>
                      <th className="px-3 py-2 font-semibold">Disable</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-proph-hairline">
                    {managers.map((manager) => (
                      <tr key={manager.user_id} className="align-top">
                        <td className="px-3 py-4 font-mono text-[13px]">
                          {manager.user_id}
                        </td>
                        <td className="px-3 py-4">{manager.email}</td>
                        <td className="px-3 py-4">{manager.phone}</td>
                        <td className="px-3 py-4">
                          {manager.disabled ? "disabled" : "active"}
                        </td>
                        <td className="px-3 py-4">
                          <form
                            action={updateManagerAccountAction}
                            className="grid min-w-[520px] grid-cols-2 gap-2"
                          >
                            <input
                              name="userId"
                              type="hidden"
                              value={manager.user_id}
                            />
                            <Input
                              name="email"
                              label="Email"
                              type="email"
                              defaultValue={manager.email}
                            />
                            <Input
                              name="phone"
                              label="Phone"
                              defaultValue={manager.phone}
                            />
                            <Input
                              name="displayName"
                              label="Display name"
                              placeholder="Optional"
                            />
                            <Input
                              name="password"
                              label="Password"
                              type="password"
                              placeholder="Optional"
                            />
                            <Input name="reason" label="Reason" required />
                            <Input
                              name="authorizationBasis"
                              label="Authorization basis"
                              required
                            />
                            <button className="h-9 bg-proph-ink px-3 text-[13px] font-semibold text-white">
                              更新
                            </button>
                          </form>
                        </td>
                        <td className="px-3 py-4">
                          <form action={disableManagerAccountAction}>
                            <input
                              name="userId"
                              type="hidden"
                              value={manager.user_id}
                            />
                            <button
                              className="h-9 border border-red-300 px-3 text-[13px] font-semibold text-red-700 disabled:cursor-not-allowed disabled:opacity-40"
                              disabled={manager.disabled}
                            >
                              停用
                            </button>
                          </form>
                        </td>
                      </tr>
                    ))}
                    {managers.length === 0 && (
                      <tr>
                        <td
                          className="px-3 py-8 text-center text-proph-ink-muted-64"
                          colSpan={6}
                        >
                          目前沒有 manager 帳號。
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </section>
          </AdminShell>
        );
      }}
    </RoleGate>
  );
}

function firstParam(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}

function messageForStatus(status?: string, error?: string): string {
  if (error === "manager-has-affiliates") {
    return "停用失敗：此 manager 仍有 active affiliate，請先重指派。";
  }
  if (error) return `操作失敗：${error}`;
  if (status === "manager-created") return "Manager 已新增。";
  if (status === "manager-updated") return "Manager 已更新。";
  if (status === "manager-disabled") return "Manager 已停用。";
  if (status === "affiliate-reassigned") return "Affiliate 已重指派。";
  return "";
}

function Input(props: {
  name: string;
  label: string;
  type?: string;
  defaultValue?: string;
  placeholder?: string;
  required?: boolean;
}) {
  return (
    <label className="block text-[12px] font-semibold text-proph-ink-muted-64">
      <span className="mb-1 block">{props.label}</span>
      <input
        className="h-10 w-full border border-proph-hairline bg-white px-3 text-[14px] text-proph-ink outline-none focus:border-proph-ink"
        name={props.name}
        type={props.type ?? "text"}
        defaultValue={props.defaultValue}
        placeholder={props.placeholder}
        required={props.required}
      />
    </label>
  );
}

function Select(props: {
  name: string;
  label: string;
  required?: boolean;
  children: ReactNode;
}) {
  return (
    <label className="block text-[12px] font-semibold text-proph-ink-muted-64">
      <span className="mb-1 block">{props.label}</span>
      <select
        className="h-10 w-full border border-proph-hairline bg-white px-3 text-[14px] text-proph-ink outline-none focus:border-proph-ink"
        name={props.name}
        required={props.required}
      >
        {props.children}
      </select>
    </label>
  );
}
