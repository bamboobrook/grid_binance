import Link from "next/link";
import { getTranslations } from "next-intl/server";
import { Bot, Zap, Shield, BarChart3, Globe, ArrowRight } from "lucide-react";

import { Button } from "@/components/ui/form";

type PageProps = {
  params: Promise<{ locale: string }>;
};

export default async function HomePage({ params }: PageProps) {
  const { locale } = await params;
  console.log('HomePage hit for locale:', locale);
  const t = await getTranslations({ locale, namespace: 'common' });

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col">
      {/* Hero Section */}
      <header className="py-20 px-6 text-center space-y-8 max-w-4xl mx-auto">
        <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-amber-500/10 border border-amber-500/20 text-amber-500 text-xs font-bold uppercase tracking-wider">
          <Zap className="w-3 h-3" />
          Powered by Binance API
        </div>
        <h1 className="text-5xl md:text-7xl font-extrabold tracking-tight leading-tight bg-gradient-to-b from-foreground to-muted-foreground bg-clip-text text-transparent">
          The Ultimate Grid <br /> 
          Trading Console
        </h1>
        <p className="text-xl text-muted-foreground max-w-2xl mx-auto leading-relaxed">
          Master the markets with professional-grade grid bots. Automate your strategy, manage risk, and track performance with 3commas-inspired precision.
        </p>
        <div className="flex flex-col sm:flex-row items-center justify-center gap-4 pt-4">
          <Link href={`/${locale}/login`}>
            <Button className="bg-amber-500 hover:bg-amber-600 text-white border-none shadow-2xl shadow-amber-500/20 px-8 py-6 text-lg rounded-2xl font-bold">
              Launch Terminal
              <ArrowRight className="w-5 h-5 ml-2" />
            </Button>
          </Link>
          <Link href={`/${locale}/register`}>
            <Button tone="secondary" className="px-8 py-6 text-lg rounded-2xl border-border">
              Create Account
            </Button>
          </Link>
        </div>
      </header>

      {/* Features Grid */}
      <section className="py-20 px-6 bg-muted/30 border-y border-border">
        <div className="max-w-7xl mx-auto grid grid-cols-1 md:grid-cols-3 gap-8">
          {[
            { 
              title: "Smart Grids", 
              desc: "Deploy sophisticated classic, long, or short grid strategies with dynamic spacing and volume control.",
              icon: Bot,
              color: "text-amber-500",
              bg: "bg-amber-500/10"
            },
            { 
              title: "Risk Guardrails", 
              desc: "Built-in pre-flight checks, hedging requirements, and real-time monitoring to protect your capital.",
              icon: Shield,
              color: "text-green-500",
              bg: "bg-green-500/10"
            },
            { 
              title: "Global Reach", 
              desc: "Full multi-language support and multi-currency tracking for traders across the globe.",
              icon: Globe,
              color: "text-blue-500",
              bg: "bg-blue-500/10"
            }
          ].map((feature, i) => (
            <div key={i} className="bg-card border border-border rounded-3xl p-8 hover:shadow-xl transition-all group">
              <div className={`w-12 h-12 rounded-2xl ${feature.bg} flex items-center justify-center mb-6 group-hover:scale-110 transition-transform`}>
                <feature.icon className={`w-6 h-6 ${feature.color}`} />
              </div>
              <h3 className="text-xl font-bold mb-3">{feature.title}</h3>
              <p className="text-muted-foreground text-sm leading-relaxed">
                {feature.desc}
              </p>
            </div>
          ))}
        </div>
      </section>

      {/* Footer */}
      <footer className="mt-auto py-10 px-6 text-center border-t border-border bg-card">
        <div className="max-w-7xl mx-auto flex flex-col md:flex-row justify-between items-center gap-6">
          <div className="flex items-center gap-2 font-bold text-lg">
            <div className="w-8 h-8 rounded-lg bg-amber-500 flex items-center justify-center text-white">
              <Bot className="w-5 h-5" />
            </div>
            Grid.Binance
          </div>
          <div className="flex gap-8 text-sm text-muted-foreground">
            <Link href="#" className="hover:text-foreground">Terms</Link>
            <Link href="#" className="hover:text-foreground">Privacy</Link>
            <Link href="#" className="hover:text-foreground">Help Center</Link>
          </div>
          <div className="text-xs text-muted-foreground italic">
            © 2026 Trading Console. Designed for professionals.
          </div>
        </div>
      </footer>
    </div>
  );
}
