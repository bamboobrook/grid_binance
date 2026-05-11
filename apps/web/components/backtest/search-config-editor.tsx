import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function SearchConfigEditor({ lang }: { lang: UiLanguage }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "搜索配置编辑器", "Search config editor")}</h3>
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "控制 symbol 池、搜索模式与配额边界。", "Control symbol pools, search modes, and quota boundaries.")}
        </p>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <fieldset className="space-y-3 rounded-xl border border-border bg-background p-4">
          <legend className="px-1 text-sm font-semibold">{pickText(lang, "Symbol pool", "Symbol pool")}</legend>
          <OptionLine description={pickText(lang, "全量筛选所有 USDT 交易对。", "Screen every USDT pair.")} groupName="symbolPool" label="all USDT" />
          <OptionLine description={pickText(lang, "只跑白名单，例如 BTCUSDT / ETHUSDT。", "Run only a curated whitelist.")} groupName="symbolPool" label="whitelist" />
          <OptionLine description={pickText(lang, "排除流动性差或不想参与的 symbol。", "Exclude thin or unwanted symbols.")} groupName="symbolPool" label="blacklist" />
        </fieldset>

        <fieldset className="space-y-3 rounded-xl border border-border bg-background p-4">
          <legend className="px-1 text-sm font-semibold">{pickText(lang, "搜索方式", "Search mode")}</legend>
          <OptionLine description={pickText(lang, "随机搜索：固定 seed，快速覆盖大范围。", "随机搜索: fixed seed, wide initial coverage.")} groupName="searchMode" label="随机搜索" />
          <OptionLine description={pickText(lang, "智能搜索：保留 Top 分位后在优区附近收缩。", "智能搜索: keep top percentile, then shrink around strong regions.")} groupName="searchMode" label="智能搜索" />
        </fieldset>
      </div>
    </section>
  );
}

function OptionLine({
  description,
  groupName,
  label,
}: {
  description: string;
  groupName: string;
  label: string;
}) {
  return (
    <label className="flex items-start gap-3 rounded-lg border border-border px-3 py-2">
      <input className="mt-1" defaultChecked={label === "all USDT" || label === "随机搜索"} name={groupName} type="radio" />
      <span>
        <span className="block text-sm font-medium">{label}</span>
        <span className="block text-xs text-muted-foreground">{description}</span>
      </span>
    </label>
  );
}
