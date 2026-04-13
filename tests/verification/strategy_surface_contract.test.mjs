import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("new strategy workspace exposes real symbol selection and strategy-type-aware controls", () => {
  const pageSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/new/page.tsx");
  const formSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-workspace-form.tsx");
  const createRoute = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/user/strategies/create/route.ts");
  const saveRoute = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/user/strategies/[id]/route.ts");
  const detailSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/[id]/page.tsx");

  assert.match(pageSource, /StrategyWorkspaceForm/, "new strategy page should delegate to the shared strategy workspace form");
  assert.match(pageSource, /max-w-\[1800px\]|grid-cols-\[/, "new strategy page should reserve a wider desktop shell for a preview + form workspace");
  assert.doesNotMatch(pageSource, /datalist id="strategy-symbol-suggestions"/, "new strategy page should no longer rely on datalist-only symbol entry");
  assert.match(formSource, /StrategySymbolPicker/, "workspace form should render a dedicated symbol picker");
  assert.match(formSource, /selectedSymbol/, "workspace form should track a selected symbol in state");
  assert.match(formSource, /strategyType/, "workspace form should track the selected strategy type");
  assert.match(formSource, /ordinary_grid/, "workspace form should expose ordinary grid as an explicit strategy type");
  assert.match(formSource, /classic_bilateral_grid/, "workspace form should expose classic bilateral grid as an explicit strategy type");
  assert.match(formSource, /Covered Range|覆盖范围/, "ordinary grid should expose a covered-range field");
  assert.match(formSource, /Upper Range|上边范围/, "classic bilateral grid should expose an upper-range field");
  assert.match(formSource, /Lower Range|下边范围/, "classic bilateral grid should expose a lower-range field");
  assert.match(formSource, /marketType !== "spot"/, "workspace form should hide futures-only controls for spot strategies");
  assert.match(formSource, /router\.(push|replace)\(/, "workspace form should support refreshing symbol-search results without posting the create form");
  assert.match(formSource, /referencePriceMode/, "workspace form should expose a reference-price source selector");
  assert.match(formSource, /data-level-editor/, "workspace form should render a real every-grid editor instead of JSON-only editing");
  assert.match(formSource, /Grid Take Profit|网格止盈/, "workspace form should keep real per-grid take-profit fields for the editor");
  assert.match(formSource, /Apply Batch Defaults|应用批量参数到逐格/, "workspace form should let operators seed the per-grid editor from batch defaults");
  assert.match(formSource, /Per-grid Base Asset Qty|单格下单币数量/, "workspace form should explain base-asset sizing in clearer terms");
  assert.match(formSource, /lg:sticky|top-/, "workspace form should keep the preview column sticky on desktop");
  assert.match(formSource, /overall.*trigger.*before|整体止盈.*先触发/s, "workspace form should warn when overall take profit can preempt grid exits");
  assert.doesNotMatch(formSource, /Field label=\{pickText\(lang, "策略模式", "Strategy Mode"\)\}/, "workspace form should no longer drive creation through the old strategy mode selector");
  assert.doesNotMatch(pageSource, /symbolMatches\[0\]\?\.symbol \?\? symbolQuery/, "new strategy page should not auto-select the first search result");
  assert.doesNotMatch(createRoute, /readField\(formData, "symbol"\) \|\| "BTCUSDT"/, "create route should not silently fall back to BTCUSDT");
  assert.match(createRoute, /search results|搜索结果/, "create route should surface an explicit symbol-selection error");
  assert.doesNotMatch(createRoute, /buildBatchLevels|buildBatchPriceLevels|resolveReferencePrice/, "create route should not generate batch levels or fetch reference prices on the server");
  assert.match(createRoute, /const strategyType = readField\(formData,\s*"strategyType"\) \|\| "ordinary_grid";/, "create route should resolve strategy_type from the submitted form field");
  assert.match(createRoute, /strategy_type:\s*strategyType/, "create route should forward strategy_type from the form");
  assert.match(createRoute, /reference_price_source:\s*mapReferencePriceSource\(readField\(formData,\s*"referencePriceMode"\)/, "create route should forward reference_price_source from the form");
  assert.match(createRoute, /levels:\s*parseLevelsJson\(readField\(formData,\s*"levels_json"\),\s*strategyType\)/, "create route should only parse submitted levels_json");
  assert.doesNotMatch(createRoute, /mapModeForStrategy|mapStrategyType|mapOrdinarySide/, "create route should forward the already-resolved mode instead of re-deriving strategy type semantics");
  assert.match(saveRoute, /const strategyType = mapStrategyType\(readField\(formData,\s*"strategyType"\)\)/, "save route should resolve strategy_type from the submitted form field");
  assert.match(saveRoute, /strategy_type:\s*strategyType/, "save route should forward strategy_type from the form");
  assert.match(saveRoute, /reference_price_source:\s*mapReferencePriceSource\(readField\(formData,\s*"referencePriceMode"\)\)/, "save route should forward reference_price_source from the form");
  assert.match(saveRoute, /levels:\s*parseLevelsJson\(readField\(formData,\s*"levels_json"\),\s*current\.draft_revision\.levels,\s*strategyType\)/, "save route should only parse submitted levels_json");
  assert.doesNotMatch(saveRoute, /buildBatchLevels|buildBatchPriceLevels|midpoint|resolveReferencePrice/, "save route should not rebuild levels or derive midpoint batches");
  assert.match(detailSource, /strategyType:/, "strategy detail page should map backend drafts into the new strategy type workspace model");
  assert.match(detailSource, /const strategyType = mapStrategyTypeToForm\(strategy\.strategy_type/, "detail page should prefer the backend strategy_type");
  assert.match(detailSource, /referencePriceMode:\s*mapReferencePriceModeToForm\(strategy\.draft_revision\.reference_price_source\)/, "detail page should map the backend reference price source");
  assert.doesNotMatch(detailSource, /fetchAnalytics|AnalyticsReport|Realized PnL|Net PnL|\/app\/analytics/, "strategy detail page should not include Task 7 analytics surfaces");
});

test("strategy preview contract distinguishes ordinary and classic bilateral layouts without TP lines", () => {
  const previewSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-visual-preview.tsx");

  assert.match(previewSource, /svg|candles|kline|candle/i, "strategy preview should render a local candle chart instead of a remote-only embed");
  assert.match(previewSource, /grid line|gridLine|entryPrice/i, "strategy preview should overlay grid levels on the chart");
  assert.match(previewSource, /strategyType/, "strategy preview should branch on the selected strategy type");
  assert.match(previewSource, /coveredRange|Covered Range|覆盖范围/i, "ordinary grid preview should describe its covered range");
  assert.match(previewSource, /centerPrice|Center Price|中心价格/i, "classic bilateral preview should describe its center anchor");
  assert.match(previewSource, /Upper Range|上边范围|Lower Range|下边范围/i, "classic bilateral preview should describe its upper and lower ranges");
  assert.match(previewSource, /data-preview-anchor|data-preview-center|data-preview-range/, "preview should expose stable summary markers for automated coverage");
  assert.doesNotMatch(previewSource, /takeProfitLines|TP Price|止盈价/i, "preview should no longer surface take-profit guide lines");
  assert.match(previewSource, /ladder|levels|preview/i, "strategy preview should summarize the configured ladder");
  assert.match(previewSource, /symbol/, "strategy preview should reflect the selected symbol");
});

test("telegram binding page exposes the bot entry link alongside bind codes", () => {
  const telegramPageSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/telegram/page.tsx");

  assert.match(telegramPageSource, /Open Telegram Bot|打开 Telegram 机器人/, "telegram page should expose a direct bot-entry action");
  assert.match(telegramPageSource, /t\.me|telegramBotLink|TELEGRAM_BOT_LINK/, "telegram page should resolve a Telegram bot URL");
  assert.match(telegramPageSource, /\/start\s*\$\{?bindCode|startParam|bindCode/, "telegram page should attach the bind code to the bot deep link");
});

test("strategy inventory exposes batch lifecycle actions and row-level actions through real forms", () => {
  const source = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/page.tsx");
  const tableSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-inventory-table.tsx");
  const detailSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/[id]/page.tsx");

  assert.match(source, /\/api\/user\/strategies\/batch/, "strategy list should post batch actions through the batch route");
  assert.match(tableSource, /value="start"/, "strategy list should expose batch start");
  assert.match(tableSource, /value="pause"/, "strategy list should expose batch pause");
  assert.match(tableSource, /value="delete"/, "strategy list should expose batch delete");
  assert.match(source, /value="stop-all"/, "strategy list should keep global stop-all");
  assert.match(tableSource, /type="checkbox"/, "strategy list should require explicit row selection checkboxes");
  assert.doesNotMatch(source, /filteredStrategies\.map\(\(strategy\) => \(\s*<input key=\{strategy\.id\} name="ids" type="hidden" value=\{strategy\.id\} \/>\s*\)\)/s, "batch actions must not submit every filtered id automatically");
  assert.match(tableSource, /\/api\/user\/strategies\/\$\{strategy\.id\}/, "strategy list should wire row actions through the detail lifecycle route");
  assert.match(source, /name="status"/, "strategy list should submit the current status filter");
  assert.match(source, /name="symbol"/, "strategy list should submit the current symbol filter");
  assert.doesNotMatch(detailSource, /if \(status === "Running"\) \{\s*return \[\s*\.\.\.common/s, "running strategies should not inherit save or pre-flight actions");
  assert.match(detailSource, /Pause Strategy|暂停策略/, "running strategies should still expose pause");
  assert.match(detailSource, /Stop Strategy|停止策略/, "running strategies should still expose stop");
});
