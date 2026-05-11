import { MartingalePortfolioDetail } from "@/components/backtest/live-portfolio-controls";
import type { UiLanguage } from "@/lib/ui/preferences";

export default async function MartingalePortfolioDetailPage({
  params,
}: {
  params: Promise<{ locale: string; id: string }>;
}) {
  const { locale, id } = await params;
  const lang = (locale === "zh" ? "zh" : "en") as UiLanguage;

  return <MartingalePortfolioDetail lang={lang} locale={locale} portfolioId={id} />;
}
