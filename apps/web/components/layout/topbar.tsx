'use client';

import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { Search, Bell, Plus, ChevronDown } from 'lucide-react';
import { Button } from '../ui/form';
import { LocaleToggle } from '../ui/locale-toggle';
import { ThemeToggle } from '../ui/theme-toggle';

export function Topbar() {
  const t = useTranslations('common.topbar');
  const locale = useLocale();

  return (
    <header className="h-14 border-b border-border bg-background/90 backdrop-blur flex items-center justify-between px-4 sticky top-0 z-20 gap-3">
      <div className="flex items-center flex-1 max-w-sm">
        <div className="relative w-full group hidden md:block">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <input
            type="text"
            placeholder={t('search')}
            className="w-full bg-input border border-border focus:border-primary/50 focus:ring-1 focus:ring-primary/20 rounded-xl pl-9 pr-3 py-1.5 text-xs outline-none transition-all placeholder:text-muted-foreground text-foreground"
          />
        </div>
      </div>

      <div className="flex items-center gap-3">
        <div className="hidden sm:flex items-center gap-2 px-3 py-1.5 bg-secondary/50 border border-border/50 rounded-xl hover:bg-secondary transition-colors">
          <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
          <span className="text-xs font-semibold text-foreground">{t('exchangeStatus')}</span>
          <ChevronDown className="w-3 h-3 text-muted-foreground" />
        </div>

        <Link href={`/${locale}/app/strategies/new`} className="hidden sm:flex">
          <Button size="sm" className="h-8 px-3 text-xs bg-primary text-primary-foreground">
            <Plus className="w-3 h-3 mr-1" /> {t('createBot')}
          </Button>
        </Link>

        <div className="h-6 w-px bg-secondary mx-1 hidden sm:block" />

        <ThemeToggle />
        <LocaleToggle />

        <button className="p-1.5 rounded-xl hover:bg-secondary text-muted-foreground transition-colors relative" title={t('connected')} type="button">
          <Bell className="w-4 h-4" />
          <span className="absolute top-1.5 right-1.5 w-1.5 h-1.5 rounded-full bg-destructive border border-background" />
        </button>

        <button className="w-8 h-8 rounded-xl bg-indigo-500/20 text-indigo-400 font-bold text-xs flex items-center justify-center hover:bg-indigo-500/30 transition-colors border border-indigo-500/30 ml-1" type="button">
          GB
        </button>
      </div>
    </header>
  );
}
