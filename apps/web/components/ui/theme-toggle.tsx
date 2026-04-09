"use client";

import { useLocale } from "next-intl";
import { Moon, Sun } from "lucide-react";
import { useTheme } from "next-themes";

import { Button } from "./form";

export function ThemeToggle() {
  const locale = useLocale();
  const { theme, setTheme } = useTheme();

  return (
    <Button
      tone="ghost"
      size="icon"
      onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
      className="text-muted-foreground hover:text-foreground"
      title={locale === "zh" ? "切换深浅模式" : "Toggle theme"}
    >
      <Sun className="h-[1.2rem] w-[1.2rem] rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
      <Moon className="absolute h-[1.2rem] w-[1.2rem] rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
      <span className="sr-only">{locale === "zh" ? "切换深浅模式" : "Toggle theme"}</span>
    </Button>
  );
}
