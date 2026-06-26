"use client";

import { Pause } from "lucide-react";

import { Button } from "@/components/ui/form";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function StopAllStrategiesForm({ lang, viewMode }: { lang: UiLanguage; viewMode: "cards" | "table" }) {
  return (
    <form action="/api/user/strategies/batch" method="post">
      <input name="intent" type="hidden" value="stop-all" />
      <input name="view" type="hidden" value={viewMode} />
      <Button className="h-8 border border-red-500/20 bg-red-500/10 px-3 text-xs text-red-500 hover:bg-red-500/20" type="submit">
        <Pause className="mr-1.5 h-3.5 w-3.5" />
        {pickText(lang, "全部停止", "Stop All")}
      </Button>
    </form>
  );
}
