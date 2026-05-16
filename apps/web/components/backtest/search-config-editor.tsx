import type { ChangeEvent } from "react";
import type { WizardForm } from "@/components/backtest/backtest-wizard";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function SearchConfigEditor({ form, lang, onChange }: { form: WizardForm; lang: UiLanguage; onChange: (event: ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) => void }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "市场搜索配置", "Market search configuration")}</h3>
        <p className="text-sm text-muted-foreground">{pickText(lang, "whitelist/blacklist 支持逗号、空格或换行分隔；白名单最多 20 个币种。", "Whitelist/blacklist accept comma, space, or newline separated symbols; whitelist supports up to 20 symbols.")}</p>
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <fieldset className="space-y-3 rounded-xl border border-border bg-background p-4">
          <legend className="px-1 text-sm font-semibold">Symbol pool</legend>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-xs uppercase tracking-wide text-muted-foreground">{pickText(lang, "池模式", "Pool mode")}</span>
            <select className="rounded-lg border border-border bg-card px-3 py-2" name="symbolPoolMode" onChange={onChange} value={form.symbolPoolMode}>
              <option value="all_usdt">all USDT</option>
              <option value="whitelist">whitelist</option>
              <option value="blacklist">blacklist</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-xs uppercase tracking-wide text-muted-foreground">whitelist</span>
            <textarea className="min-h-20 rounded-lg border border-border bg-card px-3 py-2" name="whitelist" onChange={onChange} placeholder="BTCUSDT, ETHUSDT" value={form.whitelist} />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-xs uppercase tracking-wide text-muted-foreground">blacklist</span>
            <textarea className="min-h-20 rounded-lg border border-border bg-card px-3 py-2" name="blacklist" onChange={onChange} placeholder="DOGEUSDT, PEPEUSDT" value={form.blacklist} />
          </label>
        </fieldset>
        <fieldset className="space-y-3 rounded-xl border border-border bg-background p-4">
          <legend className="px-1 text-sm font-semibold">{pickText(lang, "搜索方式", "Search mode")}</legend>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-xs uppercase tracking-wide text-muted-foreground">mode</span>
            <select className="rounded-lg border border-border bg-card px-3 py-2" name="searchMode" onChange={onChange} value={form.searchMode}>
              <option value="random">{pickText(lang, "随机搜索", "Random search")}</option>
              <option value="intelligent">{pickText(lang, "智能搜索", "Intelligent search")}</option>
            </select>
          </label>
          <div className="grid gap-3 sm:grid-cols-2">
            <NumberField label="seed" name="randomSeed" onChange={onChange} value={form.randomSeed} />
            <NumberField label={pickText(lang, "候选数量", "Candidates")} name="candidateBudget" onChange={onChange} value={form.candidateBudget} />
            <NumberField label={pickText(lang, "智能轮数", "Rounds")} name="intelligentRounds" onChange={onChange} value={form.intelligentRounds} />
            <NumberField label="Top N" name="topN" onChange={onChange} value={form.topN} />
          </div>
        </fieldset>
      </div>
    </section>
  );
}

function NumberField({ label, name, onChange, value }: { label: string; name: keyof WizardForm; onChange: (event: ChangeEvent<HTMLInputElement>) => void; value: string }) {
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span className="text-xs uppercase tracking-wide text-muted-foreground">{label}</span>
      <input className="rounded-lg border border-border bg-card px-3 py-2" min="1" name={name} onChange={onChange} type="number" value={value} />
    </label>
  );
}
