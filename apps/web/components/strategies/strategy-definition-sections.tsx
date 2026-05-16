"use client";

import { Field, Input, Select } from "@/components/ui/form";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import type { StrategyWorkspaceValues } from "@/components/strategies/strategy-workspace-form";

type DefinitionSectionsProps = {
  amountMode: StrategyWorkspaceValues["amountMode"];
  coveredRangePercent: string;
  generation: StrategyWorkspaceValues["generation"];
  gridCount: string;
  lang: UiLanguage;
  lowerRangePercent: string;
  marketType: StrategyWorkspaceValues["marketType"];
  onAmountModeChange: (value: StrategyWorkspaceValues["amountMode"]) => void;
  onCoveredRangePercentChange: (value: string) => void;
  onGenerationChange: (value: StrategyWorkspaceValues["generation"]) => void;
  onGridCountChange: (value: string) => void;
  onLowerRangePercentChange: (value: string) => void;
  onMarketTypeChange: (value: StrategyWorkspaceValues["marketType"]) => void;
  onOrdinarySideChange: (value: StrategyWorkspaceValues["ordinarySide"]) => void;
  onReferencePriceChange: (value: string) => void;
  onReferencePriceModeChange: (value: StrategyWorkspaceValues["referencePriceMode"]) => void;
  onStrategyTypeChange: (value: StrategyWorkspaceValues["strategyType"]) => void;
  onUpperRangePercentChange: (value: string) => void;
  ordinarySide: StrategyWorkspaceValues["ordinarySide"];
  referencePrice: string;
  referencePriceMode: StrategyWorkspaceValues["referencePriceMode"];
  selectedSymbol: string;
  strategyType: StrategyWorkspaceValues["strategyType"];
  upperRangePercent: string;
  values: StrategyWorkspaceValues;
};

const ordinaryGridActive = (strategyType: StrategyWorkspaceValues["strategyType"]) =>
  strategyType === "ordinary_grid";

export function StrategyDefinitionSections({
  amountMode,
  coveredRangePercent,
  generation,
  gridCount,
  lang,
  lowerRangePercent,
  marketType,
  onAmountModeChange,
  onCoveredRangePercentChange,
  onGenerationChange,
  onGridCountChange,
  onLowerRangePercentChange,
  onMarketTypeChange,
  onOrdinarySideChange,
  onReferencePriceChange,
  onReferencePriceModeChange,
  onStrategyTypeChange,
  onUpperRangePercentChange,
  ordinarySide,
  referencePrice,
  referencePriceMode,
  selectedSymbol,
  strategyType,
  upperRangePercent,
  values,
}: DefinitionSectionsProps) {
  const isOrdinary = ordinaryGridActive(strategyType);

  return (
    <div className="space-y-6">
      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        <Field label={pickText(lang, "市场类型", "Market Type")}>
          <Select
            name="marketType"
            onChange={(event) => onMarketTypeChange(event.target.value as StrategyWorkspaceValues["marketType"])}
            value={marketType}
          >
            <option value="spot">{pickText(lang, "现货", "Spot")}</option>
            <option value="usd-m">USD-M</option>
            <option value="coin-m">COIN-M</option>
          </Select>
        </Field>
        <Field label={pickText(lang, "策略类型", "Strategy Type")}>
          <Select
            name="strategyType"
            onChange={(event) => onStrategyTypeChange(event.target.value as StrategyWorkspaceValues["strategyType"])}
            value={strategyType}
          >
            <option value="ordinary_grid">{pickText(lang, "普通网格", "Ordinary Grid")}</option>
            <option value="classic_bilateral_grid">{pickText(lang, "经典双边", "Classic Bilateral Grid")}</option>
          </Select>
        </Field>
        {isOrdinary ? (
          <Field label={pickText(lang, "单侧方向", "Ordinary Side")}>
            <Select
              name="ordinarySide"
              onChange={(event) => onOrdinarySideChange(event.target.value as StrategyWorkspaceValues["ordinarySide"])}
              value={ordinarySide}
            >
              <option value="lower">{marketType === "spot" ? pickText(lang, "下侧买入", "Lower Buy Side") : pickText(lang, "下侧做多", "Lower Long Side")}</option>
              <option value="upper">{marketType === "spot" ? pickText(lang, "上侧卖出", "Upper Sell Side") : pickText(lang, "上侧做空", "Upper Short Side")}</option>
            </Select>
          </Field>
        ) : (
          <input name="ordinarySide" type="hidden" value={ordinarySide} />
        )}
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <Field label={pickText(lang, "生成方式", "Generation")}>
          <Select
            name="generation"
            onChange={(event) => onGenerationChange(event.target.value as StrategyWorkspaceValues["generation"])}
            value={generation}
          >
            <option value="arithmetic">{pickText(lang, "等差", "Arithmetic")}</option>
            <option value="geometric">{pickText(lang, "等比", "Geometric")}</option>
            <option value="custom">{pickText(lang, "完全自定义", "Custom")}</option>
          </Select>
        </Field>
        <Field label={pickText(lang, "计量模式", "Amount Mode")}>
          <Select
            name="amountMode"
            onChange={(event) => onAmountModeChange(event.target.value as StrategyWorkspaceValues["amountMode"])}
            value={amountMode}
          >
            <option value="quote">{pickText(lang, "按 USDT", "Quote Amount")}</option>
            <option value="base">{pickText(lang, "按币数量", "Base Quantity")}</option>
          </Select>
        </Field>
      </div>

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <Field label={pickText(lang, "参考价来源", "Reference Source")}>
          <Select
            onChange={(event) => {
              const nextMode = event.target.value as StrategyWorkspaceValues["referencePriceMode"];
              onReferencePriceModeChange(nextMode);
            }}
            value={referencePriceMode}
          >
            <option value="manual">{pickText(lang, "手动输入", "Manual")}</option>
            <option value="market">{pickText(lang, "当前市价", "Current Price")}</option>
          </Select>
        </Field>
        <Field
          hint={referencePriceMode === "market" ? pickText(lang, "保存时会以当前市价作为参考线", "The latest market price will be used as the reference line when saving") : undefined}
          label={pickText(lang, isOrdinary ? "锚点价格" : "中心价格", isOrdinary ? "Anchor Price" : "Center Price")}
        >
          <Input
            defaultValue={values.referencePrice}
            inputMode="decimal"
            name="referencePrice"
            onChange={(event) => onReferencePriceChange(event.target.value)}
            readOnly={referencePriceMode === "market"}
            value={referencePrice}
          />
        </Field>
        <Field label={pickText(lang, "网格数量", "Grid Count")}>
          <Input
            defaultValue={values.gridCount}
            inputMode="numeric"
            name="gridCount"
            onChange={(event) => onGridCountChange(event.target.value)}
            value={gridCount}
          />
        </Field>
        {isOrdinary ? (
          <Field label={pickText(lang, "覆盖范围 (%)", "Covered Range (%)")}>
            <Input
              defaultValue={values.coveredRangePercent}
              inputMode="decimal"
              name="coveredRangePercent"
              onChange={(event) => onCoveredRangePercentChange(event.target.value)}
              value={coveredRangePercent}
            />
          </Field>
        ) : (
          <input name="coveredRangePercent" type="hidden" value={coveredRangePercent} />
        )}
        {!isOrdinary ? (
          <>
            <Field label={pickText(lang, "上边范围 (%)", "Upper Range (%)")}>
              <Input
                defaultValue={values.upperRangePercent}
                inputMode="decimal"
                name="upperRangePercent"
                onChange={(event) => onUpperRangePercentChange(event.target.value)}
                value={upperRangePercent}
              />
            </Field>
            <Field label={pickText(lang, "下边范围 (%)", "Lower Range (%)")}>
              <Input
                defaultValue={values.lowerRangePercent}
                inputMode="decimal"
                name="lowerRangePercent"
                onChange={(event) => onLowerRangePercentChange(event.target.value)}
                value={lowerRangePercent}
              />
            </Field>
          </>
        ) : (
          <>
            <input name="upperRangePercent" type="hidden" value={upperRangePercent} />
            <input name="lowerRangePercent" type="hidden" value={lowerRangePercent} />
          </>
        )}
      </div>
    </div>
  );
}
