'use client';

import { useTranslations, useLocale } from 'next-intl';
import { usePathname, useRouter } from 'next/navigation';
import { Moon, Sun, Globe, User, Search, Settings, LogOut } from 'lucide-react';
import { useTheme } from 'next-themes';
import { cn } from '../../lib/utils';
import { Button } from '../ui/form';

export function Topbar() {
  const t = useTranslations('common.topbar');
  const locale = useLocale();
  const router = useRouter();
  const pathname = usePathname();
  const { theme, setTheme } = useTheme();

  const toggleLocale = () => {
    const nextLocale = locale === 'en' ? 'zh' : 'en';
    // Replace the current locale in the path
    const newPath = pathname.replace(`/${locale}`, `/${nextLocale}`);
    router.push(newPath);
  };

  return (
    <header className="h-16 border-b border-border bg-card/50 backdrop-blur sticky top-0 z-10 flex items-center justify-between px-6">
      <div className="flex items-center flex-1 max-w-xl">
        <div className="relative w-full group">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground group-focus-within:text-amber-500 transition-colors" />
          <input 
            type="text" 
            placeholder={t('search')} 
            className="w-full bg-muted/50 border border-transparent focus:border-amber-500/50 focus:ring-1 focus:ring-amber-500/20 rounded-full pl-10 pr-4 py-1.5 text-sm outline-none transition-all"
          />
        </div>
      </div>

      <div className="flex items-center gap-2 ml-4">
        {/* Theme Toggle */}
        <button
          onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
          className="p-2 rounded-lg hover:bg-muted text-muted-foreground transition-colors"
          aria-label="Toggle theme"
        >
          {theme === 'dark' ? <Sun className="w-5 h-5" /> : <Moon className="w-5 h-5" />}
        </button>

        {/* Language Toggle */}
        <button
          onClick={toggleLocale}
          className="flex items-center gap-2 p-2 rounded-lg hover:bg-muted text-muted-foreground transition-colors"
        >
          <Globe className="w-5 h-5" />
          <span className="text-xs font-semibold uppercase">{locale}</span>
        </button>

        <div className="h-4 w-[1px] bg-border mx-2" />

        {/* User Profile */}
        <button className="flex items-center gap-2 p-1.5 rounded-lg hover:bg-muted transition-colors">
          <div className="w-8 h-8 rounded-full bg-amber-500/20 flex items-center justify-center text-amber-500 font-bold text-xs border border-amber-500/30">
            JD
          </div>
          <div className="hidden sm:block text-left">
            <p className="text-xs font-semibold leading-none">John Doe</p>
            <p className="text-[10px] text-muted-foreground mt-0.5">Professional</p>
          </div>
        </button>
      </div>
    </header>
  );
}
