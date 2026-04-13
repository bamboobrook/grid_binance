"use client";

import { startTransition, useEffect, useState, type ReactNode } from "react";
import { Plus, Trash2 } from "lucide-react";
import { useRouter } from "next/navigation";

import { Button, Field, FormStack, Input, Select } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { StrategySymbolPicker, type StrategySymbolItem } from "@/components/strategies/strategy-symbol-picker";
import { StrategyVisualPreview, type StrategyPreviewLevel } from "@/components/strategies/strategy-visual-preview";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export type StrategyWorkspaceValues = {
  amountMode: "quote" | "base";
  baseQuantity: string;
  batchTakeProfit: string;
  batchTrailing: string;
  editorMode: "batch" | "custom";
  futuresMarginMode: "isolated" | "cross";
  generation: "arithmetic" | "geometric" | "custom";
  gridCount: string;
  gridSpacingPercent: string;
  levelsJson: string;
  leverage: string;
  marketType: "spot" | "usd-m" | "coin-m";
  mode: "classic" | "buy-only" | "sell-only" | "long" | "short" | "neutral";
  name: string;
  overallStopLoss: string;
  overallTakeProfit: string;
  postTrigger: "stop" | "rebuild";
  quoteAmount: string;
  referencePrice: string;
  referencePriceMode: "manual" | "market";
  symbol: string;
};

export type StrategyWorkspaceIntentButton = {
  tone?: "primary" | "secondary" | "danger" | "outline";
  value?: string;
  label: string;
};

type Props = {
  editingLocked?: boolean;
  formAction: string;
  intentButtons?: StrategyWorkspaceIntentButton[];
  lang: UiLanguage;
  searchPath: string;
  searchQuery: string;
  symbolMatches: StrategySymbolItem[];
  values: StrategyWorkspaceValues;
};

type EditableGridLevel = {
  id: string;
  entryPrice: string;
  spacingPercent: string;
  quantity: string;
  quoteAmount: string;
  takeProfitPercent: string;
  trailingPercent: string;
};

const SPOT_MODES: StrategyWorkspaceValues["mode"][] = ["classic", "buy-only", "sell-only"];
const FUTURES_MODES: StrategyWorkspaceValues["mode"][] = ["long", "short", "neutral"];

export function StrategyWorkspaceForm({
  editingLocked = false,
  formAction,
  intentButtons,
  lang,
  searchPath,
  searchQuery,
  symbolMatches,
  values,
}: Props) {
  const router = useRouter();
  const [query, setQuery] = useState(searchQuery);
  const [selectedSymbol, setSelectedSymbol] = useState(values.symbol);
  const [marketType, setMarketType] = useState<StrategyWorkspaceValues["marketType"]>(values.marketType);
  const [mode, setMode] = useState<StrategyWorkspaceValues["mode"]>(values.mode);
  const [generation, setGeneration] = useState<StrategyWorkspaceValues["generation"]>(values.generation);
  const [editorMode, setEditorMode] = useState<StrategyWorkspaceValues["editorMode"]>(values.editorMode);
  const [amountMode, setAmountMode] = useState<StrategyWorkspaceValues["amountMode"]>(values.amountMode);
  const [futuresMarginMode, setFuturesMarginMode] = useState<StrategyWorkspaceValues["futuresMarginMode"]>(values.futuresMarginMode);
  const [leverage, setLeverage] = useState(values.leverage);
  const [quoteAmount, setQuoteAmount] = useState(values.quoteAmount);
  const [baseQuantity, setBaseQuantity] = useState(values.baseQuantity);
  const [referencePriceMode, setReferencePriceMode] = useState<StrategyWorkspaceValues["referencePriceMode"]>(values.referencePriceMode);
  const [referencePrice, setReferencePrice] = useState(values.referencePrice);
  const [gridCount, setGridCount] = useState(values.gridCount);
  const [gridSpacingPercent, setGridSpacingPercent] = useState(values.gridSpacingPercent);
  const [batchTakeProfit, setBatchTakeProfit] = useState(values.batchTakeProfit);
  const [batchTrailing, setBatchTrailing] = useState(values.batchTrailing);
  const [overallTakeProfit, setOverallTakeProfit] = useState(values.overallTakeProfit);
  const [overallStopLoss, setOverallStopLoss] = useState(values.overallStopLoss);
  const [postTrigger, setPostTrigger] = useState(values.postTrigger);
  const [levels, setLevels] = useState<EditableGridLevel[]>(() => deriveInitialLevels(values));

  const futuresVisible = marketType !== "spot";
  const batchModeActive = editorMode === "batch" && generation !== "custom";
  const trailingWarning = batchTrailing.trim() !== "" || levels.some((level) => level.trailingPercent.trim() !== "");
  const intentRow = intentButtons ?? [{ label: pickText(lang, "创建机器人", "Create Bot") }];

  useEffect(() => {
    if (marketType === "spot" && !SPOT_MODES.includes(mode)) {
      setMode("classic");
    }
    if (marketType !== "spot" && !FUTURES_MODES.includes(mode)) {
      setMode("long");
    }
  }, [marketType, mode]);

  useEffect(() => {
    if (generation === "custom" && editorMode !== "custom") {
      setEditorMode("custom");
    }
  }, [generation, editorMode]);

  useEffect(() => {
    setSelectedSymbol(values.symbol);
  }, [values.symbol]);

  useEffect(() => {
    if (!batchModeActive) {
      return;
    }
    const generated = generateBatchEditableLevels({
      amountMode,
      baseQuantity,
      batchTakeProfit,
      batchTrailing,
      generation,
      gridCount,
      gridSpacingPercent,
      quoteAmount,
      referencePrice,
    });
    if (generated.length > 0) {
      setLevels(generated);
      setGridCount(String(generated.length));
    }
  }, [
    amountMode,
    baseQuantity,
    batchModeActive,
    batchTakeProfit,
    batchTrailing,
    generation,
    gridCount,
    gridSpacingPercent,
    levels.length,
    quoteAmount,
    referencePrice,
  ]);

  useEffect(() => {
    if (!batchModeActive) {
      setGridCount(String(levels.length || 0));
    }
  }, [batchModeActive, levels.length]);

  useEffect(() => {
    if (referencePriceMode !== "market" || !selectedSymbol) {
      return;
    }
    let cancelled = false;
    setReferencePrice("");
    fetch(marketPreviewUrl(selectedSymbol, marketType), { cache: "no-store" })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error("price fetch failed");
        }
        const payload = (await response.json()) as { latest_price?: string | null };
        if (!cancelled && typeof payload.latest_price === "string") {
          setReferencePrice(normalizeNumericString(payload.latest_price));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setReferencePrice("");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [marketType, referencePriceMode, selectedSymbol]);

  const previewLevels = toPreviewLevels(levels, amountMode);
  const levelsJson = serializeLevels(levels, amountMode);
  const referenceDisplay = referencePriceMode === "market"
    ? referencePrice || pickText(lang, "当前市价加载中", "Loading current price")
    : referencePrice;
  const canApplyBatchDefaults = canGenerateEditorSeed({
    amountMode,
    baseQuantity,
    batchTakeProfit,
    gridCount,
    gridSpacingPercent,
    quoteAmount,
    referencePrice,
  });
  const workspaceWarnings = buildWorkspaceWarnings({
    batchTakeProfit,
    lang,
    levels,
    overallTakeProfit,
  });

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)] xl:items-start">
      <div className="space-y-4 xl:sticky xl:top-24">
        <StrategyVisualPreview
          amountMode={amountMode}
          generation={generation}
          gridCount={String(levels.length || 0)}
          lang={lang}
          levels={previewLevels}
          marketType={marketType}
          mode={mode}
          referencePrice={referenceDisplay}
          selectedSymbol={selectedSymbol}
          spacingPercent={batchModeActive ? gridSpacingPercent : pickText(lang, "逐格自定义", "Per-grid custom")}
        />
      </div>

      <FormStack action={formAction} className="space-y-4 rounded-2xl border border-border bg-card p-4 shadow-sm" method="post">
        <div className="rounded-2xl border border-border/70 bg-background/60 p-4">
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {pickText(lang, "创建流程", "Build Flow")}
          </p>
          <p className="mt-1 text-sm text-foreground">
            {pickText(lang, "先锁定交易对和市场，再填写网格默认参数，最后进入逐格编辑器微调。", "Lock the symbol and market first, then fill the grid defaults and fine-tune them inside the per-grid editor.")}
          </p>
          <p className="mt-1 text-sm text-muted-foreground">
            {pickText(lang, "逐格自定义时，可以先把批量参数一键铺到全部网格，再逐格修改。", "In per-grid custom mode, you can first apply the batch defaults to every level and then edit them one by one.")}
          </p>
        </div>
        {workspaceWarnings.map((warning) => (
          <StatusBanner
            key={warning.id}
            description={warning.description}
            title={warning.title}
            tone="warning"
          />
        ))}

        <Field label={pickText(lang, "策略名称", "Strategy Name")}>
          <Input defaultValue={values.name} name="name" required />
        </Field>

        <input name="symbol" type="hidden" value={selectedSymbol} />
        <input name="referencePriceMode" type="hidden" value={referencePriceMode} />
        <input name="levels_json" type="hidden" value={levelsJson} />

        <fieldset className="contents" disabled={editingLocked}>
        <SectionBlock
          description={pickText(lang, "先从搜索结果中选中交易对，再决定现货/合约与方向。", "Choose the symbol from the search results first, then decide the market and direction.")}
          title={pickText(lang, "交易对与市场", "Symbol & Market")}
        >
        <div className="space-y-4">
        <StrategySymbolPicker
          items={symbolMatches}
          lang={lang}
          onQueryChange={(value) => {
            setQuery(value);
            setSelectedSymbol("");
          }}
          onSearch={() => {
            const next = query.trim();
            startTransition(() => {
              router.push(next ? `${searchPath}?symbolQuery=${encodeURIComponent(next)}` : searchPath);
            });
          }}
          onSelect={(item) => {
            setSelectedSymbol(item.symbol);
            setMarketType(normalizeMarket(item.market));
          }}
          query={query}
          selectedSymbol={selectedSymbol}
        />
        <div className="grid gap-3 md:grid-cols-2">
          <Field label={pickText(lang, "市场类型", "Market Type")}>
            <Select
              name="marketType"
              onChange={(event) => setMarketType(event.target.value as StrategyWorkspaceValues["marketType"])}
              value={marketType}
            >
              <option value="spot">{pickText(lang, "现货", "Spot")}</option>
              <option value="usd-m">USD-M</option>
              <option value="coin-m">COIN-M</option>
            </Select>
          </Field>
          <Field label={pickText(lang, "策略模式", "Strategy Mode")}>
            <Select
              name="mode"
              onChange={(event) => setMode(event.target.value as StrategyWorkspaceValues["mode"])}
              value={mode}
            >
              {(marketType === "spot" ? SPOT_MODES : FUTURES_MODES).map((item) => (
                <option key={item} value={item}>{describeMode(lang, item)}</option>
              ))}
            </Select>
          </Field>
        </div>
        </div>
        </SectionBlock>

        <SectionBlock
          description={pickText(lang, "决定采用批量生成还是逐格自定义，并设置每格按 USDT 还是按币数量下单。", "Choose between batch generation and per-grid editing, then decide whether each level uses quote amount or base quantity.")}
          title={pickText(lang, "建仓与计量", "Builder & Sizing")}
        >
        <div className="grid gap-3 md:grid-cols-2">
          <Field label={pickText(lang, "生成方式", "Generation")}>
            <Select
              name="generation"
              onChange={(event) => setGeneration(event.target.value as StrategyWorkspaceValues["generation"])}
              value={generation}
            >
              <option value="arithmetic">{pickText(lang, "等差", "Arithmetic")}</option>
              <option value="geometric">{pickText(lang, "等比", "Geometric")}</option>
              <option value="custom">{pickText(lang, "完全自定义", "Custom")}</option>
            </Select>
          </Field>
          <Field label={pickText(lang, "编辑模式", "Editor Mode")}>
            <Select
              name="editorMode"
              onChange={(event) => setEditorMode(event.target.value as StrategyWorkspaceValues["editorMode"])}
              value={editorMode}
            >
              <option value="batch">{pickText(lang, "批量生成", "Batch Builder")}</option>
              <option value="custom">{pickText(lang, "逐格自定义", "Per-grid Custom")}</option>
            </Select>
          </Field>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <Field label={pickText(lang, "计量模式", "Amount Mode")}>
            <Select
              name="amountMode"
              onChange={(event) => setAmountMode(event.target.value as StrategyWorkspaceValues["amountMode"])}
              value={amountMode}
            >
              <option value="quote">{pickText(lang, "按 USDT", "Quote Amount")}</option>
              <option value="base">{pickText(lang, "按币数量", "Base Quantity")}</option>
            </Select>
          </Field>
          {futuresVisible ? (
            <Field label={pickText(lang, "保证金模式", "Margin Mode")}>
              <Select
                name="futuresMarginMode"
                onChange={(event) => setFuturesMarginMode(event.target.value as StrategyWorkspaceValues["futuresMarginMode"])}
                value={futuresMarginMode}
              >
                <option value="isolated">{pickText(lang, "逐仓", "Isolated")}</option>
                <option value="cross">{pickText(lang, "全仓", "Cross")}</option>
              </Select>
            </Field>
          ) : (
            <div className="rounded-xl border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
              {pickText(lang, "现货策略不需要保证金模式。", "Spot strategies do not use margin mode.")}
            </div>
          )}
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          
          <Field hint={pickText(lang, "表示每一格下单多少基础币，只在“按币数量”模式下生效，不是通用必填项。", "This is the base-asset size used for each grid order. It only matters in Base Quantity mode and is not always required.")} label={pickText(lang, "单格下单币数量", "Per-grid Base Asset Qty")}>
            <Input
              defaultValue={values.baseQuantity}
              inputMode="decimal"
              name="baseQuantity"
              onChange={(event) => setBaseQuantity(event.target.value)}
              readOnly={amountMode !== "base"}
              value={baseQuantity}
            />
          </Field>
        </div>

        {futuresVisible ? (
          <Field label={pickText(lang, "杠杆倍数", "Leverage")}>
            <Input
              defaultValue={values.leverage}
              inputMode="numeric"
              name="leverage"
              onChange={(event) => setLeverage(event.target.value)}
              value={leverage}
            />
          </Field>
        ) : null}
        </SectionBlock>

        <SectionBlock
          description={pickText(lang, "这里配置整套网格的默认结构；逐格自定义时，也可以把这些默认值一键应用到全部网格。", "Configure the default ladder structure here. In per-grid custom mode, these defaults can also be applied to every level in one click.")}
          title={pickText(lang, "网格默认参数", "Grid Defaults")}
        >
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          <Field label={pickText(lang, "参考价来源", "Reference Source")}>
            <Select
              onChange={(event) => setReferencePriceMode(event.target.value as StrategyWorkspaceValues["referencePriceMode"])}
              value={referencePriceMode}
            >
              <option value="manual">{pickText(lang, "手动输入", "Manual")}</option>
              <option value="market">{pickText(lang, "当前市价", "Current Price")}</option>
            </Select>
          </Field>
          <Field
            hint={referencePriceMode === "market" ? pickText(lang, "保存时会以当前市价作为参考价", "The latest market price will be used when saving") : undefined}
            label={pickText(lang, "参考价格", "Reference Price")}
          >
            <Input
              defaultValue={values.referencePrice}
              inputMode="decimal"
              name="referencePrice"
              onChange={(event) => setReferencePrice(event.target.value)}
              readOnly={referencePriceMode === "market"}
              value={referencePrice}
            />
          </Field>
          <Field label={pickText(lang, "网格数量", "Grid Count")}>
            <Input
              defaultValue={values.gridCount}
              inputMode="numeric"
              name="gridCount"
              onChange={(event) => setGridCount(event.target.value)}
              value={gridCount}
            />
          </Field>
          <Field label={pickText(lang, "批量间距 (%)", "Batch Spacing (%)")}>
            <Input
              defaultValue={values.gridSpacingPercent}
              inputMode="decimal"
              name="gridSpacingPercent"
              onChange={(event) => setGridSpacingPercent(event.target.value)}
              value={gridSpacingPercent}
            />
          </Field>
          <Field hint={pickText(lang, "仅在“按 USDT”模式下生效。", "Only used in Quote Amount mode.")} label={pickText(lang, "单格投入金额 (USDT)", "Per-grid Quote Amount (USDT)")}>
            <Input
              defaultValue={values.quoteAmount}
              inputMode="decimal"
              name="quoteAmount"
              onChange={(event) => setQuoteAmount(event.target.value)}
              readOnly={amountMode !== "quote"}
              value={quoteAmount}
            />
          </Field>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <Field label={pickText(lang, "网格止盈 (%)", "Grid Take Profit (%)")}>
            <Input
              defaultValue={values.batchTakeProfit}
              inputMode="decimal"
              name="batchTakeProfit"
              onChange={(event) => setBatchTakeProfit(event.target.value)}
              value={batchTakeProfit}
            />
          </Field>
          <Field label={pickText(lang, "追踪止盈 (%)", "Trailing Take Profit (%)")}>
            <Input
              defaultValue={values.batchTrailing}
              inputMode="decimal"
              name="batchTrailing"
              onChange={(event) => setBatchTrailing(event.target.value)}
              value={batchTrailing}
            />
          </Field>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-dashed border-border/70 bg-muted/20 px-4 py-3">
          <div className="space-y-1">
            <p className="text-sm font-semibold text-foreground">
              {pickText(lang, "逐格自定义前，先批量铺满全部网格", "Seed every level before manual editing")}
            </p>
            <p className="text-sm text-muted-foreground">
              {pickText(lang, "会按当前网格数量、间距、每格金额/币量、网格止盈与追踪止盈，重建下方逐格编辑器。", "This rebuilds the per-grid editor from the current grid count, spacing, per-grid amount/base size, grid take profit, and trailing take profit.")}
            </p>
          </div>
          <Button
            disabled={!canApplyBatchDefaults}
            onClick={() => {
              const seeded = generateEditorSeedLevels({
                amountMode,
                baseQuantity,
                batchTakeProfit,
                batchTrailing,
                generation,
                gridCount,
                gridSpacingPercent,
                quoteAmount,
                referencePrice,
              });
              if (seeded.length > 0) {
                setEditorMode("custom");
                setLevels(seeded);
                setGridCount(String(seeded.length));
              }
            }}
            tone="outline"
            type="button"
          >
            {pickText(lang, "应用批量参数到逐格", "Apply Batch Defaults")}
          </Button>
        </div>
        </SectionBlock>

        <details className="group mt-4 rounded-2xl border border-border bg-card p-2 open:bg-background/50 transition-colors" open>
          <summary className="cursor-pointer px-4 py-3 text-sm font-bold text-foreground hover:text-primary outline-none marker:text-primary">
            {pickText(lang, "高级风控与逐格设置 (Advanced Risk & Editor)", "Advanced Risk & Editor")}
          </summary>
          <div className="space-y-6 pt-4 px-2 pb-2">
        {trailingWarning ? (
          <div className="rounded-xl border border-amber-300 bg-amber-50 px-3 py-3 text-sm text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-100">
            {pickText(lang, "已启用追踪止盈：这会改用 taker 方式止盈，手续费通常高于 maker，请确认后再启动。", "Trailing TP is enabled: this switches to taker exits, which usually cost more than maker orders. Review the fees before starting.")}
          </div>
        ) : null}

        <SectionBlock
          description={pickText(lang, "整体止盈止损作用于整套策略，不是单个网格。", "Overall take profit and stop loss apply to the whole strategy, not a single level.")}
          title={pickText(lang, "整体风控", "Portfolio Risk")}
        >
        <div className="grid gap-3 md:grid-cols-2">
          <Field label={pickText(lang, "整体止盈 (%)", "Overall Take Profit (%)")}>
            <Input
              defaultValue={values.overallTakeProfit}
              inputMode="decimal"
              name="overallTakeProfit"
              onChange={(event) => setOverallTakeProfit(event.target.value)}
              value={overallTakeProfit}
            />
          </Field>
          <Field hint={pickText(lang, "留空表示不启用整体止损", "Leave empty to disable overall stop loss")} label={pickText(lang, "整体止损 (%)", "Overall Stop Loss (%)")}>
            <Input
              defaultValue={values.overallStopLoss}
              inputMode="decimal"
              name="overallStopLoss"
              onChange={(event) => setOverallStopLoss(event.target.value)}
              value={overallStopLoss}
            />
          </Field>
        </div>

        <Field label={pickText(lang, "触发后行为", "Post Trigger Action")}>
          <Select name="postTrigger" onChange={(event) => setPostTrigger(event.target.value as StrategyWorkspaceValues["postTrigger"])} value={postTrigger}>
            <option value="stop">{pickText(lang, "执行后停止", "Stop After Trigger")}</option>
            <option value="rebuild">{pickText(lang, "重建继续", "Rebuild and Continue")}</option>
          </Select>
        </Field>
        </SectionBlock>

        <div className="space-y-3 rounded-2xl border border-border bg-background/40 p-4" data-level-editor="true">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {pickText(lang, "逐格编辑器", "Per-grid Editor")}
              </p>
              <p className="text-sm text-muted-foreground">
                {batchModeActive
                  ? pickText(lang, "当前仍会根据上面的批量参数自动生成阶梯；切换到逐格自定义后，可逐行修改每一格。", "The ladder is still generated from the batch controls above. Switch to per-grid custom to edit every row manually.")
                  : pickText(lang, "逐行设置每一格的入场价、相邻间距、投入金额、网格止盈和追踪止盈。", "Edit each level row-by-row, including entry price, spacing, amount, grid take profit, and trailing take profit.")}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Button onClick={() => {
                setEditorMode("custom");
                setLevels((current) => addEditableLevel(current, amountMode, gridSpacingPercent, referencePrice));
              }} size="sm" tone="outline" type="button">
                <Plus className="mr-1.5 h-3.5 w-3.5" />
                {pickText(lang, "新增一格", "Add Level")}
              </Button>
            </div>
          </div>

          <div className="space-y-3">
            {levels.map((level, index) => {
              const secondaryAmount = amountMode === "quote"
                ? pickText(lang, `约 ${displayRowQuantity(level)} 币`, `Approx. ${displayRowQuantity(level)} units`)
                : pickText(lang, `约 ${displayRowQuote(level)} USDT`, `Approx. ${displayRowQuote(level)} USDT`);
              return (
                <div className="rounded-2xl border border-border bg-card p-3" key={level.id}>
                  <div className="grid gap-3 xl:grid-cols-[0.65fr_1fr_1fr_1fr_1fr_1fr_auto]">
                    <div className="space-y-1">
                      <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">L{index + 1}</div>
                      <div className="text-xs text-muted-foreground">
                        {index === 0
                          ? pickText(lang, "第一格", "First level")
                          : pickText(lang, `相邻间距 ${level.spacingPercent || "-"}%`, `Spacing ${level.spacingPercent || "-"}%`)}
                      </div>
                    </div>
                    <Field label={pickText(lang, "入场价", "Entry Price")}>
                      <Input
                        inputMode="decimal"
                        onChange={(event) => setLevels((current) => updateLevelField(current, index, "entryPrice", event.target.value, amountMode))}
                        readOnly={batchModeActive}
                        value={level.entryPrice}
                      />
                    </Field>
                    <Field label={pickText(lang, "与上一格间距 (%)", "Spacing vs Prev (%)")}>
                      <Input
                        inputMode="decimal"
                        onChange={(event) => setLevels((current) => updateLevelSpacing(current, index, event.target.value, amountMode))}
                        readOnly={index === 0}
                        value={index === 0 ? "" : level.spacingPercent}
                      />
                    </Field>
                    <Field label={amountMode === "quote" ? pickText(lang, "投入金额 (USDT)", "Quote Amount (USDT)") : pickText(lang, "币数量", "Base Quantity")}>
                      <Input
                        inputMode="decimal"
                        onChange={(event) => setLevels((current) => updateLevelField(current, index, amountMode === "quote" ? "quoteAmount" : "quantity", event.target.value, amountMode))}
                        readOnly={batchModeActive}
                        value={amountMode === "quote" ? level.quoteAmount : level.quantity}
                      />
                    </Field>
                    <Field hint={secondaryAmount} label={pickText(lang, "网格止盈 (%)", "Grid Take Profit (%)")}>
                      <Input
                        inputMode="decimal"
                        onChange={(event) => setLevels((current) => updateLevelField(current, index, "takeProfitPercent", event.target.value, amountMode))}
                        readOnly={batchModeActive}
                        value={level.takeProfitPercent}
                      />
                    </Field>
                    <Field label={pickText(lang, "追踪止盈 (%)", "Trailing Take Profit (%)")}>
                      <Input
                        inputMode="decimal"
                        onChange={(event) => setLevels((current) => updateLevelField(current, index, "trailingPercent", event.target.value, amountMode))}
                        readOnly={batchModeActive}
                        value={level.trailingPercent}
                      />
                    </Field>
                    <div className="flex items-end justify-end gap-2">
                      <Button
                        disabled={levels.length <= 1 && !batchModeActive}
                        onClick={() => {
                          setEditorMode("custom");
                          setLevels((current) => removeEditableLevel(current, index, amountMode));
                        }}
                        size="icon"
                        tone="outline"
                        type="button"
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
        
          </div>
        </details>
        </fieldset>

        <div className="flex flex-wrap gap-2">
          {intentRow.map((button, index) => (
            <Button
              key={`${button.label}-${index}`}
              name={button.value ? "intent" : undefined}
              tone={button.tone ?? (button.value === "delete" ? "danger" : button.value === "pause" || button.value === "stop" ? "outline" : "primary")}
              type="submit"
              value={button.value}
            >
              {button.label}
            </Button>
          ))}
        </div>
      </FormStack>
    </div>
  );
}

function SectionBlock({
  children,
  description,
  title,
}: {
  children: ReactNode;
  description?: string;
  title: string;
}) {
  return (
    <section className="space-y-4 rounded-2xl border border-border/70 bg-background/45 p-4">
      <div className="space-y-1">
        <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{title}</p>
        {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
      </div>
      {children}
    </section>
  );
}

function describeMode(lang: UiLanguage, mode: StrategyWorkspaceValues["mode"]) {
  switch (mode) {
    case "buy-only":
      return pickText(lang, "只买", "Buy Only");
    case "sell-only":
      return pickText(lang, "只卖", "Sell Only");
    case "long":
      return pickText(lang, "做多", "Long");
    case "short":
      return pickText(lang, "做空", "Short");
    case "neutral":
      return pickText(lang, "中性", "Neutral");
    default:
      return pickText(lang, "经典", "Classic");
  }
}

function normalizeMarket(market: string): StrategyWorkspaceValues["marketType"] {
  if (market === "coinm") {
    return "coin-m";
  }
  if (market === "usdm") {
    return "usd-m";
  }
  return "spot";
}

function deriveInitialLevels(values: StrategyWorkspaceValues): EditableGridLevel[] {
  const parsed = parseLevelsJson(values.levelsJson);
  if (parsed.length > 0) {
    return normalizeEditableLevels(parsed, values.amountMode);
  }
  const generated = generateBatchEditableLevels({
    amountMode: values.amountMode,
    baseQuantity: values.baseQuantity,
    batchTakeProfit: values.batchTakeProfit,
    batchTrailing: values.batchTrailing,
    generation: values.generation,
    gridCount: values.gridCount,
    gridSpacingPercent: values.gridSpacingPercent,
    quoteAmount: values.quoteAmount,
    referencePrice: values.referencePrice,
  });
  if (generated.length > 0) {
    return generated;
  }
  return [createEditableLevel(0, {
    entryPrice: values.referencePrice,
    quantity: values.baseQuantity,
    quoteAmount: values.quoteAmount,
    takeProfitPercent: values.batchTakeProfit,
    trailingPercent: values.batchTrailing,
  })];
}

function parseLevelsJson(raw: string): EditableGridLevel[] {
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .map((item, index) => {
        const entryPrice = readString(item?.entry_price);
        const quantity = readString(item?.quantity);
        const takeProfitBps = Number.parseFloat(String(item?.take_profit_bps ?? ""));
        const trailingBps = item?.trailing_bps == null ? null : Number.parseFloat(String(item.trailing_bps));
        if (!entryPrice || !quantity || !Number.isFinite(takeProfitBps) || takeProfitBps <= 0) {
          return null;
        }
        const quoteAmount = deriveQuoteAmount(entryPrice, quantity);
        return createEditableLevel(index, {
          entryPrice,
          quantity,
          quoteAmount,
          takeProfitPercent: formatPercent(takeProfitBps / 100),
          trailingPercent: trailingBps !== null && Number.isFinite(trailingBps) && trailingBps > 0 ? formatPercent(trailingBps / 100) : "",
        });
      })
      .filter((item): item is EditableGridLevel => item !== null);
  } catch {
    return [];
  }
}

function generateBatchEditableLevels(input: {
  amountMode: StrategyWorkspaceValues["amountMode"];
  baseQuantity: string;
  batchTakeProfit: string;
  batchTrailing: string;
  generation: StrategyWorkspaceValues["generation"];
  gridCount: string;
  gridSpacingPercent: string;
  quoteAmount: string;
  referencePrice: string;
}): EditableGridLevel[] {
  if (input.generation === "custom") {
    return [];
  }

  const count = Number.parseInt(input.gridCount, 10);
  const reference = Number.parseFloat(input.referencePrice);
  const spacing = Number.parseFloat(input.gridSpacingPercent);
  const takeProfit = Number.parseFloat(input.batchTakeProfit);
  const trailing = input.batchTrailing.trim() === "" ? null : Number.parseFloat(input.batchTrailing);
  const quoteAmount = Number.parseFloat(input.quoteAmount);
  const baseQuantity = Number.parseFloat(input.baseQuantity);

  if (!Number.isFinite(count) || count <= 0 || !Number.isFinite(reference) || reference <= 0 || !Number.isFinite(spacing) || spacing <= 0 || !Number.isFinite(takeProfit) || takeProfit <= 0) {
    return [];
  }

  const midpoint = (count - 1) / 2;
  const items: EditableGridLevel[] = [];
  for (let index = 0; index < count; index += 1) {
    const offset = index - midpoint;
    const spacingFactor = spacing / 100;
    const price = input.generation === "geometric"
      ? reference * Math.pow(1 + spacingFactor, offset)
      : reference * (1 + spacingFactor * offset);
    if (!Number.isFinite(price) || price <= 0) {
      continue;
    }
    const resolvedQuote = input.amountMode === "quote" ? quoteAmount : price * baseQuantity;
    const resolvedQuantity = input.amountMode === "quote" ? resolvedQuote / price : baseQuantity;
    if (!Number.isFinite(resolvedQuantity) || resolvedQuantity <= 0 || !Number.isFinite(resolvedQuote) || resolvedQuote <= 0) {
      continue;
    }
    items.push(createEditableLevel(index, {
      entryPrice: formatDecimal(price),
      quantity: formatDecimal(resolvedQuantity),
      quoteAmount: formatDecimal(resolvedQuote),
      takeProfitPercent: formatPercent(takeProfit),
      trailingPercent: trailing !== null && Number.isFinite(trailing) && trailing > 0 ? formatPercent(trailing) : "",
    }));
  }
  return applyUniformSpacing(normalizeEditableLevels(items, input.amountMode), input.gridSpacingPercent);
}

function generateEditorSeedLevels(input: {
  amountMode: StrategyWorkspaceValues["amountMode"];
  baseQuantity: string;
  batchTakeProfit: string;
  batchTrailing: string;
  generation: StrategyWorkspaceValues["generation"];
  gridCount: string;
  gridSpacingPercent: string;
  quoteAmount: string;
  referencePrice: string;
}) {
  if (input.generation !== "custom") {
    return generateBatchEditableLevels(input);
  }

  const count = Number.parseInt(input.gridCount, 10);
  const reference = Number.parseFloat(input.referencePrice);
  const spacing = Number.parseFloat(input.gridSpacingPercent);
  const takeProfit = Number.parseFloat(input.batchTakeProfit);
  const trailing = input.batchTrailing.trim() === "" ? null : Number.parseFloat(input.batchTrailing);
  const quoteAmount = Number.parseFloat(input.quoteAmount);
  const baseQuantity = Number.parseFloat(input.baseQuantity);

  if (!Number.isFinite(count) || count <= 0 || !Number.isFinite(reference) || reference <= 0 || !Number.isFinite(spacing) || spacing <= 0 || !Number.isFinite(takeProfit) || takeProfit <= 0) {
    return [];
  }

  const midpoint = (count - 1) / 2;
  const spacingFactor = spacing / 100;
  const items: EditableGridLevel[] = [];
  for (let index = 0; index < count; index += 1) {
    const offset = index - midpoint;
    const price = reference * Math.pow(1 + spacingFactor, offset);
    if (!Number.isFinite(price) || price <= 0) {
      continue;
    }
    const resolvedQuote = input.amountMode === "quote" ? quoteAmount : price * baseQuantity;
    const resolvedQuantity = input.amountMode === "quote" ? resolvedQuote / price : baseQuantity;
    if (!Number.isFinite(resolvedQuantity) || resolvedQuantity <= 0 || !Number.isFinite(resolvedQuote) || resolvedQuote <= 0) {
      continue;
    }
    items.push(createEditableLevel(index, {
      entryPrice: formatDecimal(price),
      quantity: formatDecimal(resolvedQuantity),
      quoteAmount: formatDecimal(resolvedQuote),
      takeProfitPercent: formatPercent(takeProfit),
      trailingPercent: trailing !== null && Number.isFinite(trailing) && trailing > 0 ? formatPercent(trailing) : "",
    }));
  }
  return normalizeEditableLevels(items, input.amountMode);
}

function canGenerateEditorSeed(input: {
  amountMode: StrategyWorkspaceValues["amountMode"];
  baseQuantity: string;
  batchTakeProfit: string;
  gridCount: string;
  gridSpacingPercent: string;
  quoteAmount: string;
  referencePrice: string;
}) {
  const count = Number.parseInt(input.gridCount, 10);
  const reference = Number.parseFloat(input.referencePrice);
  const spacing = Number.parseFloat(input.gridSpacingPercent);
  const takeProfit = Number.parseFloat(input.batchTakeProfit);
  const amount = input.amountMode === "quote"
    ? Number.parseFloat(input.quoteAmount)
    : Number.parseFloat(input.baseQuantity);

  return Number.isFinite(count) && count > 0
    && Number.isFinite(reference) && reference > 0
    && Number.isFinite(spacing) && spacing > 0
    && Number.isFinite(takeProfit) && takeProfit > 0
    && Number.isFinite(amount) && amount > 0;
}

function applyUniformSpacing(levels: EditableGridLevel[], spacingPercent: string) {
  const parsedSpacing = Number.parseFloat(spacingPercent);
  const normalizedSpacing = Number.isFinite(parsedSpacing) ? formatPercent(parsedSpacing) : "";
  return levels.map((level, index) => {
    if (index === 0 || !normalizedSpacing) {
      return level;
    }
    return {
      ...level,
      spacingPercent: normalizedSpacing,
    };
  });
}

function createEditableLevel(index: number, partial?: Partial<EditableGridLevel>): EditableGridLevel {
  return {
    id: partial?.id ?? `level-${index + 1}-${Math.random().toString(36).slice(2, 10)}`,
    entryPrice: partial?.entryPrice ?? "",
    spacingPercent: partial?.spacingPercent ?? "",
    quantity: partial?.quantity ?? "",
    quoteAmount: partial?.quoteAmount ?? "",
    takeProfitPercent: partial?.takeProfitPercent ?? "",
    trailingPercent: partial?.trailingPercent ?? "",
  };
}

function normalizeEditableLevels(levels: EditableGridLevel[], amountMode: StrategyWorkspaceValues["amountMode"]) {
  return levels.map((level, index) => {
    const prev = levels[index - 1];
    const entryPrice = Number.parseFloat(level.entryPrice);
    const quantity = Number.parseFloat(level.quantity);
    const quoteAmount = Number.parseFloat(level.quoteAmount);
    let nextQuantity = level.quantity;
    let nextQuoteAmount = level.quoteAmount;

    if (amountMode === "quote" && Number.isFinite(entryPrice) && entryPrice > 0 && Number.isFinite(quoteAmount) && quoteAmount > 0) {
      nextQuantity = formatDecimal(quoteAmount / entryPrice);
    }
    if (amountMode === "base" && Number.isFinite(entryPrice) && entryPrice > 0 && Number.isFinite(quantity) && quantity > 0) {
      nextQuoteAmount = formatDecimal(entryPrice * quantity);
    }

    return {
      ...level,
      quantity: nextQuantity,
      quoteAmount: nextQuoteAmount,
      spacingPercent: index === 0 ? "" : computeSpacingPercent(prev?.entryPrice ?? "", level.entryPrice),
    };
  });
}

function updateLevelField(
  levels: EditableGridLevel[],
  index: number,
  field: keyof Pick<EditableGridLevel, "entryPrice" | "quantity" | "quoteAmount" | "takeProfitPercent" | "trailingPercent">,
  value: string,
  amountMode: StrategyWorkspaceValues["amountMode"],
) {
  const next = levels.map((level, currentIndex) => currentIndex === index ? { ...level, [field]: value } : { ...level });
  return normalizeEditableLevels(next, amountMode);
}

function updateLevelSpacing(
  levels: EditableGridLevel[],
  index: number,
  value: string,
  amountMode: StrategyWorkspaceValues["amountMode"],
) {
  if (index === 0) {
    return levels;
  }
  const next = levels.map((level) => ({ ...level }));
  next[index].spacingPercent = value;
  const previousPrice = Number.parseFloat(next[index - 1].entryPrice);
  const spacing = Number.parseFloat(value);
  if (Number.isFinite(previousPrice) && previousPrice > 0 && Number.isFinite(spacing) && spacing > -100) {
    next[index].entryPrice = formatDecimal(previousPrice * (1 + spacing / 100));
  }
  return normalizeEditableLevels(next, amountMode);
}

function addEditableLevel(
  levels: EditableGridLevel[],
  amountMode: StrategyWorkspaceValues["amountMode"],
  fallbackSpacing: string,
  fallbackReferencePrice: string,
) {
  const next = levels.map((level) => ({ ...level }));
  const last = next[next.length - 1];
  const lastPrice = Number.parseFloat(last?.entryPrice ?? fallbackReferencePrice);
  const spacing = Number.parseFloat(last?.spacingPercent || fallbackSpacing || "1");
  const nextPrice = Number.isFinite(lastPrice) && lastPrice > 0
    ? lastPrice * (1 + ((Number.isFinite(spacing) ? spacing : 1) / 100))
    : Number.parseFloat(fallbackReferencePrice || "0");
  next.push(createEditableLevel(next.length, {
    entryPrice: Number.isFinite(nextPrice) && nextPrice > 0 ? formatDecimal(nextPrice) : "",
    quantity: last?.quantity ?? "",
    quoteAmount: last?.quoteAmount ?? "",
    takeProfitPercent: last?.takeProfitPercent ?? "",
    trailingPercent: last?.trailingPercent ?? "",
  }));
  return normalizeEditableLevels(next, amountMode);
}

function removeEditableLevel(levels: EditableGridLevel[], index: number, amountMode: StrategyWorkspaceValues["amountMode"]) {
  if (levels.length <= 1) {
    return levels;
  }
  return normalizeEditableLevels(levels.filter((_, currentIndex) => currentIndex !== index), amountMode);
}

function serializeLevels(levels: EditableGridLevel[], amountMode: StrategyWorkspaceValues["amountMode"]) {
  return JSON.stringify(levels.map((level) => {
    const entryPrice = Number.parseFloat(level.entryPrice);
    const quantity = amountMode === "quote"
      ? resolveQuoteModeQuantity(level.entryPrice, level.quoteAmount)
      : level.quantity.trim();
    const takeProfitPercent = Number.parseFloat(level.takeProfitPercent);
    const trailingPercent = level.trailingPercent.trim() === "" ? null : Number.parseFloat(level.trailingPercent);
    return {
      entry_price: Number.isFinite(entryPrice) && entryPrice > 0 ? normalizeNumericString(level.entryPrice) : level.entryPrice.trim(),
      quantity,
      take_profit_bps: Number.isFinite(takeProfitPercent) && takeProfitPercent > 0 ? Math.round(takeProfitPercent * 100) : 0,
      trailing_bps: trailingPercent !== null && Number.isFinite(trailingPercent) && trailingPercent > 0 ? Math.round(trailingPercent * 100) : null,
    };
  }), null, 2);
}

function resolveQuoteModeQuantity(entryPriceRaw: string, quoteAmountRaw: string) {
  const entryPrice = Number.parseFloat(entryPriceRaw);
  const quoteAmount = Number.parseFloat(quoteAmountRaw);
  if (!Number.isFinite(entryPrice) || entryPrice <= 0 || !Number.isFinite(quoteAmount) || quoteAmount <= 0) {
    return "";
  }
  return formatDecimal(quoteAmount / entryPrice);
}

function toPreviewLevels(levels: EditableGridLevel[], amountMode: StrategyWorkspaceValues["amountMode"]): StrategyPreviewLevel[] {
  return levels
    .map((level) => {
      const quantity = amountMode === "quote" ? resolveQuoteModeQuantity(level.entryPrice, level.quoteAmount) : level.quantity.trim();
      if (!level.entryPrice.trim() || !quantity || !level.takeProfitPercent.trim()) {
        return null;
      }
      return {
        entryPrice: level.entryPrice.trim(),
        quantity,
        spacingPercent: level.spacingPercent.trim() || null,
        takeProfitPercent: level.takeProfitPercent.trim(),
        trailingPercent: level.trailingPercent.trim() || null,
      } satisfies StrategyPreviewLevel;
    })
    .filter((item): item is StrategyPreviewLevel => item !== null);
}

function deriveQuoteAmount(entryPriceRaw: string, quantityRaw: string) {
  const entryPrice = Number.parseFloat(entryPriceRaw);
  const quantity = Number.parseFloat(quantityRaw);
  if (!Number.isFinite(entryPrice) || entryPrice <= 0 || !Number.isFinite(quantity) || quantity <= 0) {
    return "";
  }
  return formatDecimal(entryPrice * quantity);
}

function displayRowQuantity(level: EditableGridLevel) {
  return resolveQuoteModeQuantity(level.entryPrice, level.quoteAmount) || level.quantity || "-";
}

function displayRowQuote(level: EditableGridLevel) {
  return deriveQuoteAmount(level.entryPrice, level.quantity) || level.quoteAmount || "-";
}

function computeSpacingPercent(previousRaw: string, currentRaw: string) {
  const previous = Number.parseFloat(previousRaw);
  const current = Number.parseFloat(currentRaw);
  if (!Number.isFinite(previous) || previous <= 0 || !Number.isFinite(current) || current <= 0) {
    return "";
  }
  return formatPercent(((current - previous) / previous) * 100);
}

function readString(value: unknown) {
  if (typeof value === "number") {
    return normalizeNumericString(String(value));
  }
  return typeof value === "string" ? value.trim() : "";
}

function normalizeNumericString(value: string) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) {
    return value.trim();
  }
  return formatDecimal(parsed);
}

function formatDecimal(value: number) {
  const normalized = value.toFixed(8).replace(/\.0+$/, "").replace(/(\.\d*?)0+$/, "$1");
  return normalized === "-0" ? "0" : normalized;
}

function formatPercent(value: number) {
  return value.toFixed(4).replace(/0+$/, "").replace(/\.$/, "");
}

function marketPreviewUrl(symbol: string, marketType: StrategyWorkspaceValues["marketType"]) {
  const params = new URLSearchParams({
    marketType,
    symbol: symbol.trim().toUpperCase(),
  });
  return `/api/market/preview?${params.toString()}`;
}

function buildWorkspaceWarnings(input: {
  batchTakeProfit: string;
  lang: UiLanguage;
  levels: EditableGridLevel[];
  overallTakeProfit: string;
}) {
  const warnings: Array<{ id: string; title: string; description: string }> = [];
  const overallTakeProfit = Number.parseFloat(input.overallTakeProfit);
  const initialGridTakeProfit = Number.parseFloat(input.batchTakeProfit);
  const minGridTakeProfit = input.levels.reduce<number | null>((current, level) => {
    const next = Number.parseFloat(level.takeProfitPercent);
    if (!Number.isFinite(next) || next <= 0) {
      return current;
    }
    if (current === null) {
      return next;
    }
    return Math.min(current, next);
  }, Number.isFinite(initialGridTakeProfit) && initialGridTakeProfit > 0 ? initialGridTakeProfit : null);

  if (
    Number.isFinite(overallTakeProfit)
    && overallTakeProfit > 0
    && minGridTakeProfit !== null
    && overallTakeProfit <= minGridTakeProfit
  ) {
    warnings.push({
      description: pickText(
        input.lang,
        `当前整体止盈为 ${formatPercent(overallTakeProfit)}%，最小网格止盈为 ${formatPercent(minGridTakeProfit)}%。整体止盈可能先触发，导致单格止盈计划还没走完就整套平仓。`,
        `Overall take profit is ${formatPercent(overallTakeProfit)}%, while the smallest grid take profit is ${formatPercent(minGridTakeProfit)}%. Overall take profit may trigger before the grid take-profit plan finishes.`,
      ),
      id: "overall-vs-grid-tp",
      title: pickText(
        input.lang,
        "整体止盈可能先于网格止盈触发",
        "Overall take profit may trigger before the grid take-profit plan",
      ),
    });
  }

  return warnings;
}
