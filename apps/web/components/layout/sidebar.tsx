'use client';

import { useTranslations } from 'next-intl';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { 
  LayoutDashboard, 
  Bot, 
  History, 
  ArrowLeftRight, 
  HelpCircle,
  Settings,
  Bell,
  LineChart,
  Wallet
} from 'lucide-react';
import { cn } from '@/lib/utils';

export function Sidebar() {
  const t = useTranslations('common.sidebar');
  const pathname = usePathname();

  const navItems = [
    { name: t('dashboard'), href: '/app/dashboard', icon: LayoutDashboard },
    { name: 'My Portfolio', href: '/app/portfolio', icon: Wallet },
    { name: 'SmartTrade', href: '/app/smart-trade', icon: LineChart },
    { name: 'DCA Bots', href: '/app/dca', icon: Bot },
    { name: 'Grid Bots', href: '/app/strategies', icon: ArrowLeftRight },
    { name: t('orders'), href: '/app/orders', icon: History },
  ];

  const bottomItems = [
    { name: t('exchange'), href: '/app/exchange', icon: Wallet },
    { name: t('settings'), href: '/app/settings', icon: Settings },
  ];

  const isCurrent = (href: string) => {
    // Check if the current pathname includes the href
    return pathname.includes(href);
  };

  return (
    <aside className="w-60 bg-[#0a101d] border-r border-border hidden md:flex flex-col text-slate-300">
      {/* Brand */}
      <div className="h-14 flex items-center px-4 font-bold text-white tracking-wide border-b border-border/50">
        <div className="w-6 h-6 rounded bg-primary flex items-center justify-center mr-3 text-white">
          <Bot className="w-4 h-4" />
        </div>
        Grid.Binance
      </div>

      {/* Main Navigation */}
      <nav className="flex-1 py-4 px-2 space-y-1 overflow-y-auto">
        <div className="text-[10px] font-bold text-slate-500 uppercase tracking-wider mb-2 px-2">Trading</div>
        {navItems.map((item) => {
          const active = isCurrent(item.href);
          return (
            <Link
              key={item.href}
              href={`/en${item.href}`} // We assume EN for now, middleware will handle it
              className={cn(
                "flex items-center gap-3 px-3 py-2 rounded-sm text-sm font-medium transition-colors",
                active 
                  ? "bg-primary/10 text-primary" 
                  : "hover:bg-slate-800/50 hover:text-white"
              )}
            >
              <item.icon className={cn("w-4 h-4", active ? "text-primary" : "text-slate-400")} />
              {item.name}
            </Link>
          );
        })}
      </nav>

      {/* Bottom Actions */}
      <div className="p-2 space-y-1 border-t border-border/50">
        {bottomItems.map((item) => (
          <Link
            key={item.href}
            href={`/en${item.href}`}
            className="flex items-center gap-3 px-3 py-2 rounded-sm text-sm font-medium transition-colors hover:bg-slate-800/50 hover:text-white"
          >
            <item.icon className="w-4 h-4 text-slate-400" />
            {item.name}
          </Link>
        ))}
        <Link
          href="/en/help"
          className="flex items-center gap-3 px-3 py-2 rounded-sm text-sm font-medium transition-colors hover:bg-slate-800/50 hover:text-white mt-4 text-slate-400"
        >
          <HelpCircle className="w-4 h-4" />
          {t('help')}
        </Link>
      </div>
    </aside>
  );
}
