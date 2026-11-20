import { ProphBadge } from "@prophetica/ui";

export function GuardianWidget() {
  return (
    <div className="relative mx-auto w-full max-w-2xl overflow-hidden rounded-proph-xl border border-white/10 bg-[#161618]/90 p-6 shadow-2xl backdrop-blur-2xl sm:p-8">
      {/* Top telemetry */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 pb-4">
        <div className="flex items-center gap-2">
          <ProphBadge tone="success" size="sm" pulse>
            GUARDIAN ACTIVE
          </ProphBadge>
          <span className="font-text text-[13px] text-zinc-300">
            責任博彩守護體系 · SPEC-021
          </span>
        </div>
        <div className="font-mono text-[12px] text-emerald-400">
          帳戶健康評等: A+ 健全
        </div>
      </div>

      {/* Control Grid */}
      <div className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="rounded-proph-md border border-white/10 bg-white/5 p-4">
          <div className="flex items-center justify-between text-[12px] text-zinc-400 mb-1.5">
            <span>單月儲值上限</span>
            <span className="text-emerald-400 font-semibold">自訂已啟用</span>
          </div>
          <div className="font-mono text-[18px] font-bold text-white">
            NT$ 5,000 / 月
          </div>
          <p className="mt-1 text-[11px] text-zinc-400">
            達到上限時系統將自動鎖定，杜絕非理性支出
          </p>
        </div>

        <div className="rounded-proph-md border border-white/10 bg-white/5 p-4">
          <div className="flex items-center justify-between text-[12px] text-zinc-400 mb-1.5">
            <span>智能冷靜期開關</span>
            <span className="text-blue-400 font-semibold">隨時可觸發</span>
          </div>
          <div className="font-mono text-[18px] font-bold text-white">
            24h ~ 30 天
          </div>
          <p className="mt-1 text-[11px] text-zinc-400">
            一鍵暫停交易活動，合約期間不可撤銷
          </p>
        </div>
      </div>

      {/* Bottom Legal Notice */}
      <div className="mt-6 flex flex-wrap items-center justify-between gap-3 border-t border-white/10 pt-4 text-[12px] text-zinc-400">
        <div className="flex items-center gap-2">
          <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
          <span>未滿 18 歲禁止參與 · 理性預測，量力而行</span>
        </div>
        <div className="font-mono text-zinc-500">24/7 諮詢專線: 1925</div>
      </div>
    </div>
  );
}
