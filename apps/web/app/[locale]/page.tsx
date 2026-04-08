import Link from "next/link";
import { getTranslations } from "next-intl/server";
import { Bot, LineChart, ShieldCheck, ChevronRight, Activity, Zap, Lock } from "lucide-react";

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
    <div className="min-h-screen bg-[#0a0e17] text-slate-200 flex flex-col font-sans selection:bg-primary/30">
      {/* Top Navigation */}
      <nav className="flex items-center justify-between px-6 py-4 max-w-7xl mx-auto w-full z-10 border-b border-slate-800/50 bg-[#0a0e17]/80 backdrop-blur-md sticky top-0">
        <div className="flex items-center gap-3 font-extrabold text-2xl tracking-tight text-white">
          <div className="w-9 h-9 rounded-lg bg-primary flex items-center justify-center text-primary-foreground shadow-lg shadow-primary/30">
            <Bot className="w-6 h-6" />
          </div>
          Grid.Binance
        </div>
        <div className="flex items-center gap-5">
          <ThemeToggle />
          <LocaleToggle />
          <Link href={`/${locale}/login`} className="text-sm font-semibold text-slate-400 hover:text-white transition-colors hidden sm:block">
            {t('viewDemo') || "Log in"}
          </Link>
          <Link href={`/${locale}/register`}>
            <Button className="bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg px-6 h-10 text-sm font-bold shadow-lg shadow-primary/20 transition-all">
              {t('startFree') || "Start free trial"}
            </Button>
          </Link>
        </div>
      </nav>

      {/* Hero Section */}
      <header className="flex-1 flex flex-col items-center justify-center text-center px-4 py-24 relative overflow-hidden">
        {/* Background Gradients */}
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-primary/10 rounded-full blur-[120px] pointer-events-none" />
        <div className="absolute top-1/4 left-1/4 w-[400px] h-[400px] bg-indigo-500/10 rounded-full blur-[100px] pointer-events-none" />

        <div className="relative z-10 flex flex-col items-center">
          <div className="inline-flex items-center gap-2 px-4 py-1.5 rounded-full bg-[#111827] border border-slate-800 text-slate-300 text-xs font-bold mb-10 shadow-sm">
            <Activity className="w-3.5 h-3.5 text-primary" />
            {t('advancedGrid') || "Advanced Grid Trading Bots"}
          </div>
          
          <h1 className="text-6xl md:text-8xl font-extrabold text-white tracking-tight leading-[1.05] max-w-5xl mx-auto mb-8 drop-shadow-sm">
            {t('title1')} <span className="text-transparent bg-clip-text bg-gradient-to-r from-primary to-indigo-400">{t('title2')}</span>
          </h1>
          
          <p className="text-xl md:text-2xl text-slate-400 max-w-3xl mx-auto mb-12 leading-relaxed">
            {t('subtitle')}
          </p>
          
          <div className="flex flex-col sm:flex-row items-center gap-5 w-full sm:w-auto">
            <Link href={`/${locale}/register`} className="w-full sm:w-auto">
              <Button className="w-full sm:w-auto bg-primary hover:bg-primary/90 text-primary-foreground px-10 h-16 text-lg rounded-xl font-bold shadow-xl shadow-primary/25 group transition-all transform hover:-translate-y-0.5">
                {t('startFree')}
                <ChevronRight className="w-5 h-5 ml-2 group-hover:translate-x-1.5 transition-transform" />
              </Button>
            </Link>
            <Link href={`/${locale}/login`} className="w-full sm:w-auto">
              <Button className="w-full sm:w-auto px-10 h-16 text-lg rounded-xl font-bold border border-slate-700 bg-[#111827] text-white hover:bg-slate-800 hover:border-slate-600 transition-all">
                {t('viewDemo')}
              </Button>
            </Link>
          </div>
        </div>
      </header>

      {/* Feature Highlights */}
      <section className="py-28 bg-[#0a0e17] border-t border-slate-800 relative z-10">
        <div className="max-w-7xl mx-auto px-6 grid grid-cols-1 md:grid-cols-3 gap-12">
          <div className="space-y-5 p-8 rounded-2xl bg-[#111827] border border-slate-800 hover:border-slate-700 transition-colors">
            <div className="w-12 h-12 rounded-xl bg-indigo-500/10 flex items-center justify-center border border-indigo-500/20">
              <LineChart className="w-6 h-6 text-indigo-400" />
            </div>
            <h3 className="text-2xl font-bold text-white">{t('features.smartTrade.title')}</h3>
            <p className="text-slate-400 leading-relaxed text-base">
              {t('features.smartTrade.desc')}
            </p>
          </div>
          <div className="space-y-5 p-8 rounded-2xl bg-[#111827] border border-slate-800 hover:border-slate-700 transition-colors relative overflow-hidden">
            <div className="absolute top-0 right-0 w-32 h-32 bg-emerald-500/5 rounded-bl-full pointer-events-none" />
            <div className="w-12 h-12 rounded-xl bg-emerald-500/10 flex items-center justify-center border border-emerald-500/20">
              <Zap className="w-6 h-6 text-emerald-400" />
            </div>
            <h3 className="text-2xl font-bold text-white">{t('features.dcaBots.title')}</h3>
            <p className="text-slate-400 leading-relaxed text-base">
              {t('features.dcaBots.desc')}
            </p>
          </div>
          <div className="space-y-5 p-8 rounded-2xl bg-[#111827] border border-slate-800 hover:border-slate-700 transition-colors">
            <div className="w-12 h-12 rounded-xl bg-amber-500/10 flex items-center justify-center border border-amber-500/20">
              <ShieldCheck className="w-6 h-6 text-amber-400" />
            </div>
            <h3 className="text-2xl font-bold text-white">{t('features.riskManagement.title')}</h3>
            <p className="text-slate-400 leading-relaxed text-base">
              {t('features.riskManagement.desc')}
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}
