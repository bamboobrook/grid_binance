"use client";

import { useState, type ReactNode } from "react";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { cn } from "@/lib/utils";

type WizardStep = {
  key: string;
  label: string;
  labelEn: string;
};

const STEPS: WizardStep[] = [
  { key: "symbol", label: "交易对与类型", labelEn: "Symbol & Type" },
  { key: "grid", label: "网格参数", labelEn: "Grid Params" },
  { key: "risk", label: "风控设置", labelEn: "Risk Control" },
  { key: "confirm", label: "确认预览", labelEn: "Confirm" },
];

export function StrategyWizard({
  lang,
  children,
}: {
  lang: UiLanguage;
  children: (step: string, goTo: (dir: "next" | "prev") => void, current: number, total: number) => ReactNode;
}) {
  const [currentStep, setCurrentStep] = useState(0);

  const goTo = (dir: "next" | "prev") => {
    if (dir === "next" && currentStep < STEPS.length - 1) {
      setCurrentStep(currentStep + 1);
    } else if (dir === "prev" && currentStep > 0) {
      setCurrentStep(currentStep - 1);
    }
  };

  return (
    <div className="space-y-4">
      {/* Step indicator */}
      <div className="flex items-center gap-1 sm:gap-2">
        {STEPS.map((step, i) => (
          <div key={step.key} className="flex items-center gap-1 sm:gap-2 flex-1">
            <button
              type="button"
              onClick={() => setCurrentStep(i)}
              className={cn(
                "flex items-center gap-1.5 rounded-lg px-2 sm:px-3 py-1.5 text-xs sm:text-sm font-medium transition-colors w-full",
                i === currentStep
                  ? "bg-primary/10 text-primary border border-primary/20"
                  : i < currentStep
                    ? "bg-emerald-500/10 text-emerald-500 border border-emerald-500/20"
                    : "bg-muted text-muted-foreground border border-border",
              )}
            >
              <span
                className={cn(
                  "flex h-5 w-5 sm:h-6 sm:w-6 shrink-0 items-center justify-center rounded-full text-[10px] sm:text-xs font-bold",
                  i === currentStep
                    ? "bg-primary text-primary-foreground"
                    : i < currentStep
                      ? "bg-emerald-500 text-white"
                      : "bg-muted-foreground/20 text-muted-foreground",
                )}
              >
                {i < currentStep ? "✓" : i + 1}
              </span>
              <span className="truncate hidden sm:inline">{pickText(lang, step.label, step.labelEn)}</span>
            </button>
            {i < STEPS.length - 1 && (
              <div className={cn("h-px flex-1 max-w-[20px] sm:max-w-[40px]", i < currentStep ? "bg-emerald-500" : "bg-border")} />
            )}
          </div>
        ))}
      </div>

      {/* Step content */}
      <div className="min-h-[400px]">
        {children(STEPS[currentStep].key, goTo, currentStep, STEPS.length)}
      </div>

      {/* Navigation buttons */}
      <div className="flex items-center justify-between pt-2 border-t border-border">
        <button
          type="button"
          onClick={() => goTo("prev")}
          disabled={currentStep === 0}
          className={cn(
            "rounded-lg px-4 py-2.5 text-sm font-medium transition-colors",
            currentStep === 0
              ? "text-muted-foreground cursor-not-allowed"
              : "bg-secondary text-foreground hover:bg-secondary/80",
          )}
        >
          {pickText(lang, "上一步", "Previous")}
        </button>
        <span className="text-xs text-muted-foreground">
          {currentStep + 1} / {STEPS.length}
        </span>
        <button
          type="button"
          onClick={() => goTo("next")}
          disabled={currentStep === STEPS.length - 1}
          className={cn(
            "rounded-lg px-4 py-2.5 text-sm font-medium transition-colors",
            currentStep === STEPS.length - 1
              ? "text-muted-foreground cursor-not-allowed"
              : "bg-primary text-primary-foreground hover:bg-primary/90",
          )}
        >
          {pickText(lang, "下一步", "Next")}
        </button>
      </div>
    </div>
  );
}
