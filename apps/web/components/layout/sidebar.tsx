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
  Bell
} from 'lucide-react';
import { cn } from '../../lib/utils';

export function Sidebar() {
  const t = useTranslations('common.sidebar');
  const pathname = usePathname();

  const navItems = [
    { icon: LayoutDashboard, label: t('dashboard'), href: '/dashboard' },
    { icon: Bot, label: t('strategies'), href: '/strategies' },
    { icon: History, label: t('orders'), href: '/orders' },
    { icon: ArrowLeftRight, label: t('exchange'), href: '/exchange' },
  ];

  return (
    <aside className="w-64 bg-card border-r border-border h-screen sticky top-0 flex flex-col">
      <div className="p-6">
        <h1 className="text-xl font-bold tracking-tighter flex items-center gap-2">
          <Bot className="w-8 h-8 text-amber-500" />
          <span className="brand-mark">GRID BOT</span>
        </h1>
      </div>

      <nav className="flex-1 px-4 py-4 space-y-1">
        {navItems.map((item) => {
          const isActive = pathname.includes(item.href);
          return (
            <Link
              key={item.href}
              href={item.href}
              className={cn(
                "flex items-center gap-3 px-3 py-2 rounded-lg transition-colors",
                isActive 
                  ? "bg-amber-500/10 text-amber-500 border border-amber-500/30" 
                  : "text-muted-foreground hover:bg-muted hover:text-foreground"
              )}
            >
              <item.icon className="w-5 h-5" />
              <span className="font-medium text-sm">{item.label}</span>
            </Link>
          );
        })}
      </nav>

      <div className="p-4 border-t border-border space-y-1">
        <Link href="/notifications" className="flex items-center gap-3 px-3 py-2 text-muted-foreground hover:text-foreground">
          <Bell className="w-5 h-5" />
          <span className="text-sm">Notifications</span>
        </Link>
        <Link href="/settings" className="flex items-center gap-3 px-3 py-2 text-muted-foreground hover:text-foreground">
          <Settings className="w-5 h-5" />
          <span className="text-sm">Settings</span>
        </Link>
        <Link href="/help" className="flex items-center gap-3 px-3 py-2 text-muted-foreground hover:text-foreground">
          <HelpCircle className="w-5 h-5" />
          <span className="text-sm">Help Center</span>
        </Link>
      </div>
    </aside>
  );
}
