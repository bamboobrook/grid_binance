import { BacktestConsole } from "@/components/backtest/backtest-console";
import type { UiLanguage } from "@/lib/ui/preferences";

export default async function BacktestPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const lang = (locale === "zh" ? "zh" : "en") as UiLanguage;

  return <BacktestConsole lang={lang} locale={locale} />;
}
