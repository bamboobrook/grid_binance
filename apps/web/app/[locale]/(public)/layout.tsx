import type { ReactNode } from "react";
import { getTranslations } from "next-intl/server";
import { Bot } from "lucide-react";
import Link from "next/link";

export default async function PublicLayout({
  children,
  params
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  
  return (
    <div className="min-h-screen bg-[#0a101d] text-slate-200 flex flex-col font-sans selection:bg-primary/30">
      {/* Simple Header */}
      <nav className="flex items-center justify-between px-6 py-4 w-full border-b border-slate-800/50 bg-[#0f172a]/50 backdrop-blur">
        <Link href={`/${locale}`} className="flex items-center gap-2 font-bold text-xl text-white tracking-tight">
          <div className="w-8 h-8 rounded bg-primary flex items-center justify-center text-white shadow-lg shadow-primary/20">
            <Bot className="w-5 h-5" />
          </div>
          Grid.Binance
        </Link>
      </nav>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col items-center justify-center p-4">
        {children}
      </main>

      {/* Simple Footer */}
      <footer className="py-6 text-center text-xs text-slate-500">
        © 2026 Grid Trading Console. Designed for professionals.
      </footer>
    </div>
  );
}
