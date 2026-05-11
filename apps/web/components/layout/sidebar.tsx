'use client';

import { useLocale, useTranslations } from 'next-intl';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  LayoutDashboard,
  ArrowLeftRight,
  History,
  Wallet,
  ShieldCheck,
  Bell,
  BarChart3,
  HelpCircle,
  Bot,
  Layers3,
  FlaskConical,
} from 'lucide-react';
import { cn } from '@/lib/utils';

export function Sidebar() {
  const t = useTranslations('common.sidebar');
  const locale = useLocale();
  const pathname = usePathname();
  const backtestLabel = locale === "zh" ? "回测" : "Backtest";
  const martingalePortfoliosLabel = locale === "zh" ? "马丁组合" : "Martingale Portfolios";

  const navItems = [
    { name: t('dashboard'), href: '/app/dashboard', icon: LayoutDashboard },
    { name: t('strategies'), href: '/app/strategies', icon: ArrowLeftRight },
    { name: t('orders'), href: '/app/orders', icon: History },
    { name: t('analytics'), href: '/app/analytics', icon: BarChart3 },
    { name: backtestLabel, href: '/app/backtest', icon: FlaskConical },
    { name: martingalePortfoliosLabel, href: '/app/martingale-portfolios', icon: Layers3 },
  ];

  const bottomItems = [
    { name: t('exchange'), href: '/app/exchange', icon: Wallet },
    { name: t('notifications'), href: '/app/telegram', icon: Bell },
    { name: t('settings'), href: '/app/security', icon: ShieldCheck },
  ];

  const isCurrent = (href: string) => pathname === `/${locale}${href}` || pathname.startsWith(`/${locale}${href}/`);

  return (
    <aside className="hidden md:flex w-64 shrink-0 flex-col border-r border-border bg-card/80 backdrop-blur text-foreground">
      <div className="h-16 flex items-center px-4 font-bold text-foreground tracking-wide border-b border-border/50">
        <div className="w-8 h-8 rounded-xl bg-primary/15 border border-primary/20 flex items-center justify-center mr-3 text-primary">
          <Bot className="w-4 h-4" />
        </div>
        <div className="flex flex-col">
          <span className="text-sm font-extrabold tracking-tight">Grid.Binance</span>
          <span className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">SaaS Console</span>
        </div>
      </div>

      <nav className="flex-1 py-5 px-3 space-y-1 overflow-y-auto">
        <div className="text-[10px] font-bold text-muted-foreground uppercase tracking-[0.18em] mb-3 px-2">{t('trading')}</div>
        {navItems.map((item) => {
          const active = isCurrent(item.href);
          return (
            <Link
              key={item.href}
              aria-current={active ? "page" : undefined}
              href={`/${locale}${item.href}`}
              className={cn(
                'flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition-colors',
                active ? 'bg-primary/10 text-primary ring-1 ring-primary/20' : 'text-muted-foreground hover:bg-secondary/70 hover:text-foreground',
              )}
            >
              <item.icon className={cn('w-4 h-4', active ? 'text-primary' : 'text-muted-foreground')} />
              {item.name}
            </Link>
          );
        })}
      </nav>

      <div className="p-3 space-y-1 border-t border-border/50 bg-background/50">
        {bottomItems.map((item) => {
          const active = isCurrent(item.href);
          return (
            <Link
              key={item.href}
              aria-current={active ? "page" : undefined}
              href={`/${locale}${item.href}`}
              className={cn(
                'flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition-colors',
                active ? 'bg-primary/10 text-primary ring-1 ring-primary/20' : 'text-muted-foreground hover:bg-secondary/70 hover:text-foreground',
              )}
            >
              <item.icon className={cn('w-4 h-4', active ? 'text-primary' : 'text-muted-foreground')} />
              {item.name}
            </Link>
          );
        })}
        <Link
          aria-current={isCurrent('/app/help') ? "page" : undefined}
          href={`/${locale}/app/help`}
          className={cn(
            'mt-4 flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition-colors',
            isCurrent('/app/help') ? 'bg-primary/10 text-primary ring-1 ring-primary/20' : 'text-muted-foreground hover:bg-secondary/70 hover:text-foreground',
          )}
        >
          <HelpCircle className="w-4 h-4" />
          {t('help')}
        </Link>
      </div>
    </aside>
  );
}
