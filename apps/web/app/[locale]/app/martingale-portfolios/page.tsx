import { MartingalePortfolioList } from "@/components/backtest/live-portfolio-controls";
import type { UiLanguage } from "@/lib/ui/preferences";

export default async function MartingalePortfoliosPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const lang = (locale === "zh" ? "zh" : "en") as UiLanguage;

  return <MartingalePortfolioList lang={lang} locale={locale} />;
}
