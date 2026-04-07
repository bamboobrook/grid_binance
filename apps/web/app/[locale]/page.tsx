import Link from "next/link";
import { getTranslations } from "next-intl/server";
import { Bot, LineChart, ShieldCheck, ChevronRight, Activity } from "lucide-react";

import { Button } from "@/components/ui/form";

type PageProps = {
  params: Promise<{ locale: string }>;
};

export default async function HomePage({ params }: PageProps) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'common' });

  return (
    <div className="min-h-screen bg-[#0f172a] text-slate-200 flex flex-col font-sans selection:bg-primary/30">
      {/* Top Navigation */}
      <nav className="flex items-center justify-between px-6 py-4 max-w-7xl mx-auto w-full">
        <div className="flex items-center gap-2 font-bold text-xl text-white tracking-tight">
          <div className="w-8 h-8 rounded bg-primary flex items-center justify-center text-white shadow-lg shadow-primary/20">
            <Bot className="w-5 h-5" />
          </div>
          3Commas Clone
        </div>
        <div className="flex items-center gap-4">
          <Link href={`/${locale}/login`} className="text-sm font-medium text-slate-300 hover:text-white transition-colors hidden sm:block">
            Log in
          </Link>
          <Link href={`/${locale}/register`}>
            <Button className="bg-primary hover:bg-primary/90 text-white rounded-sm px-5 h-9 text-sm font-semibold shadow-md">
              Start free trial
            </Button>
          </Link>
        </div>
      </nav>

      {/* Hero Section */}
      <header className="flex-1 flex flex-col items-center justify-center text-center px-4 py-20 mt-10">
        <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-slate-800 border border-slate-700 text-slate-300 text-xs font-semibold mb-8">
          <Activity className="w-3 h-3 text-primary" />
          Advanced Grid & DCA Bots
        </div>
        
        <h1 className="text-5xl md:text-7xl font-extrabold text-white tracking-tight leading-[1.1] max-w-4xl mx-auto mb-6">
          Automate your crypto trading <span className="text-primary">like a pro.</span>
        </h1>
        
        <p className="text-lg md:text-xl text-slate-400 max-w-2xl mx-auto mb-10 leading-relaxed">
          Maximize profits and minimize risks. Build, test, and deploy algorithmic trading strategies on Binance and other major exchanges in minutes.
        </p>
        
        <div className="flex flex-col sm:flex-row items-center gap-4 w-full sm:w-auto">
          <Link href={`/${locale}/register`} className="w-full sm:w-auto">
            <Button className="w-full sm:w-auto bg-primary hover:bg-primary/90 text-white px-8 h-14 text-base rounded-sm font-bold shadow-lg shadow-primary/20 group">
              Start trading for free
              <ChevronRight className="w-4 h-4 ml-1 group-hover:translate-x-1 transition-transform" />
            </Button>
          </Link>
          <Link href={`/${locale}/login`} className="w-full sm:w-auto">
            <Button className="w-full sm:w-auto px-8 h-14 text-base rounded-sm font-semibold border-slate-700 text-slate-300 hover:bg-slate-800 hover:text-white">
              View Demo Account
            </Button>
          </Link>
        </div>
      </header>

      {/* Feature Highlights */}
      <section className="py-24 bg-[#0a101d] border-t border-slate-800/50 mt-auto">
        <div className="max-w-7xl mx-auto px-6 grid grid-cols-1 md:grid-cols-3 gap-12">
          <div className="space-y-4">
            <div className="w-10 h-10 rounded bg-indigo-500/10 flex items-center justify-center border border-indigo-500/20">
              <LineChart className="w-5 h-5 text-indigo-400" />
            </div>
            <h3 className="text-xl font-bold text-white">SmartTrade Terminal</h3>
            <p className="text-slate-400 leading-relaxed text-sm">
              Execute advanced orders with built-in Take Profit and Stop Loss. Manage your entire portfolio from a single, powerful interface.
            </p>
          </div>
          <div className="space-y-4">
            <div className="w-10 h-10 rounded bg-emerald-500/10 flex items-center justify-center border border-emerald-500/20">
              <Bot className="w-5 h-5 text-emerald-400" />
            </div>
            <h3 className="text-xl font-bold text-white">Grid & DCA Bots</h3>
            <p className="text-slate-400 leading-relaxed text-sm">
              Profit from market volatility 24/7. Run classic, long, or short strategies completely hands-free with optimized algorithms.
            </p>
          </div>
          <div className="space-y-4">
            <div className="w-10 h-10 rounded bg-amber-500/10 flex items-center justify-center border border-amber-500/20">
              <ShieldCheck className="w-5 h-5 text-amber-400" />
            </div>
            <h3 className="text-xl font-bold text-white">Risk Management</h3>
            <p className="text-slate-400 leading-relaxed text-sm">
              Set global portfolio safeguards, trailed stops, and timeout rules. Your funds stay safe on the exchange via secure API connections.
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}
