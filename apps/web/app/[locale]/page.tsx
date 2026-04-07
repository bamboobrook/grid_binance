import Link from "next/link";
import { getTranslations } from "next-intl/server";
import { Bot, LineChart, ShieldCheck, ChevronRight, Activity } from "lucide-react";

import { Button } from "@/components/ui/form";
import { LocaleToggle } from "@/components/ui/locale-toggle";
import { ThemeToggle } from "@/components/ui/theme-toggle";

type PageProps = {
  params: Promise<{ locale: string }>;
};

export default async function HomePage({ params }: PageProps) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'home' });

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col font-sans selection:bg-primary/30">
      {/* Top Navigation */}
      <nav className="flex items-center justify-between px-6 py-4 max-w-7xl mx-auto w-full">
        <div className="flex items-center gap-2 font-bold text-xl tracking-tight">
          <div className="w-8 h-8 rounded bg-primary flex items-center justify-center text-primary-foreground shadow-lg shadow-primary/20">
            <Bot className="w-5 h-5" />
          </div>
          Grid.Binance
        </div>
        <div className="flex items-center gap-4">
          <ThemeToggle />
          <LocaleToggle />
          <Link href={`/${locale}/login`} className="text-sm font-medium text-muted-foreground hover:text-foreground transition-colors hidden sm:block">
            Log in
          </Link>
          <Link href={`/${locale}/register`}>
            <Button className="bg-primary hover:bg-primary/90 text-primary-foreground rounded-sm px-5 h-9 text-sm font-semibold shadow-md">
              Start free trial
            </Button>
          </Link>
        </div>
      </nav>

      {/* Hero Section */}
      <header className="flex-1 flex flex-col items-center justify-center text-center px-4 py-20 mt-10">
        <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-secondary border border-border text-foreground text-xs font-semibold mb-8">
          <Activity className="w-3 h-3 text-primary" />
          {t('advancedGrid')}
        </div>
        
        <h1 className="text-5xl md:text-7xl font-extrabold text-foreground tracking-tight leading-[1.1] max-w-4xl mx-auto mb-6">
          {t('title1')} <span className="text-primary">{t('title2')}</span>
        </h1>
        
        <p className="text-lg md:text-xl text-muted-foreground max-w-2xl mx-auto mb-10 leading-relaxed">
          {t('subtitle')}
        </p>
        
        <div className="flex flex-col sm:flex-row items-center gap-4 w-full sm:w-auto">
          <Link href={`/${locale}/register`} className="w-full sm:w-auto">
            <Button className="w-full sm:w-auto bg-primary hover:bg-primary/90 text-foreground px-8 h-14 text-base rounded-sm font-bold shadow-lg shadow-primary/20 group">
              {t('startFree')}
              <ChevronRight className="w-4 h-4 ml-1 group-hover:translate-x-1 transition-transform" />
            </Button>
          </Link>
          <Link href={`/${locale}/login`} className="w-full sm:w-auto">
            <Button className="w-full sm:w-auto px-8 h-14 text-base rounded-sm font-semibold border-border text-foreground hover:bg-secondary hover:text-foreground">
              {t('viewDemo')}
            </Button>
          </Link>
        </div>
      </header>

      {/* Feature Highlights */}
      <section className="py-24 bg-muted border-t border-border/50 mt-auto">
        <div className="max-w-7xl mx-auto px-6 grid grid-cols-1 md:grid-cols-3 gap-12">
          <div className="space-y-4">
            <div className="w-10 h-10 rounded bg-indigo-500/10 flex items-center justify-center border border-indigo-500/20">
              <LineChart className="w-5 h-5 text-indigo-400" />
            </div>
            <h3 className="text-xl font-bold text-foreground">{t('features.smartTrade.title')}</h3>
            <p className="text-muted-foreground leading-relaxed text-sm">
              {t('features.smartTrade.desc')}
            </p>
          </div>
          <div className="space-y-4">
            <div className="w-10 h-10 rounded bg-emerald-500/10 flex items-center justify-center border border-emerald-500/20">
              <Bot className="w-5 h-5 text-emerald-400" />
            </div>
            <h3 className="text-xl font-bold text-foreground">{t('features.dcaBots.title')}</h3>
            <p className="text-muted-foreground leading-relaxed text-sm">
              {t('features.dcaBots.desc')}
            </p>
          </div>
          <div className="space-y-4">
            <div className="w-10 h-10 rounded bg-amber-500/10 flex items-center justify-center border border-amber-500/20">
              <ShieldCheck className="w-5 h-5 text-amber-400" />
            </div>
            <h3 className="text-xl font-bold text-foreground">{t('features.riskManagement.title')}</h3>
            <p className="text-muted-foreground leading-relaxed text-sm">
              {t('features.riskManagement.desc')}
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}
