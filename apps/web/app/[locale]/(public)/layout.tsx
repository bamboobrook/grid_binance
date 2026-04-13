import type { ReactNode } from "react";
import { Bot } from "lucide-react";
import Link from "next/link";
import { LocaleToggle } from "@/components/ui/locale-toggle";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { pickText } from "@/lib/ui/preferences";

export default async function PublicLayout({
  children,
  params
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const lang = locale === "en" ? "en" : "zh";
  
  return (
    <div className="min-h-screen bg-muted text-foreground flex flex-col font-sans selection:bg-primary/30">
      {/* Simple Header */}
      <nav className="flex items-center justify-between px-6 py-4 w-full border-b border-border/50 bg-background/50 backdrop-blur">
        <Link href={`/${locale}`} className="flex items-center gap-2 font-bold text-xl text-foreground tracking-tight">
          <div className="w-8 h-8 rounded bg-primary flex items-center justify-center text-primary-foreground shadow-lg shadow-primary/20">
            <Bot className="w-5 h-5" />
          </div>
          Grid.Binance
        </Link>
        <div className="flex items-center gap-2">
          <ThemeToggle />
          <LocaleToggle />
        </div>
      </nav>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col items-center justify-center p-4">
        {children}
      </main>

      {/* Simple Footer */}
      <footer className="py-6 text-center text-xs text-muted-foreground">
        {pickText(lang, "© 2026 Grid Trading Console。面向专业交易者。", "© 2026 Grid Trading Console. Designed for professionals.")}
      </footer>
    </div>
  );
}
