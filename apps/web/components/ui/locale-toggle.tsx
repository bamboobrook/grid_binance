"use client";

import { useLocale } from "next-intl";
import { usePathname, useRouter } from "next/navigation";
import { Globe } from "lucide-react";

import { buildPreferenceCookie, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";
import { Button } from "./form";

export function LocaleToggle() {
  const locale = useLocale();
  const router = useRouter();
  const pathname = usePathname();

  const toggleLocale = () => {
    const nextLocale = locale === "en" ? "zh" : "en";
    document.cookie = buildPreferenceCookie(UI_LANGUAGE_COOKIE, nextLocale);
    const newPath = pathname.replace(/^\/(zh|en)(?=\/|$)/, `/${nextLocale}`);
    router.push(newPath);
  };

  return (
    <Button
      tone="ghost"
      size="default"
      onClick={toggleLocale}
      className="text-muted-foreground hover:text-foreground font-semibold uppercase text-xs"
      title={locale === "zh" ? "切换语言" : "Toggle language"}
    >
      <Globe className="w-4 h-4 mr-2" />
      {locale}
    </Button>
  );
}
