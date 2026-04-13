import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("new strategy workspace exposes real symbol selection and market-aware controls", () => {
  const pageSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/new/page.tsx");
  const formSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-workspace-form.tsx");
  const createRoute = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/user/strategies/create/route.ts");

  assert.match(pageSource, /StrategyWorkspaceForm/, "new strategy page should delegate to the shared strategy workspace form");
  assert.match(pageSource, /max-w-\[1800px\]|grid-cols-\[/, "new strategy page should reserve a wider desktop shell for a preview + form workspace");
  assert.doesNotMatch(pageSource, /datalist id=\"strategy-symbol-suggestions\"/, "new strategy page should no longer rely on datalist-only symbol entry");
  assert.match(formSource, /StrategySymbolPicker/, "workspace form should render a dedicated symbol picker");
  assert.match(formSource, /selectedSymbol/, "workspace form should track a selected symbol in state");
  assert.match(formSource, /marketType !== "spot"/, "workspace form should hide futures-only controls for spot strategies");
  assert.match(formSource, /router\.(push|replace)\(/, "workspace form should support refreshing symbol-search results without posting the create form");
  assert.match(formSource, /referencePriceMode/, "workspace form should expose a reference-price source selector");
  assert.match(formSource, /data-level-editor/, "workspace form should render a real every-grid editor instead of JSON-only editing");
  assert.match(formSource, /Grid Take Profit|网格止盈/, "workspace form should describe per-grid take profit rather than only batch take profit");
  assert.match(formSource, /Apply Batch Defaults|应用批量参数到逐格/, "workspace form should let operators seed the per-grid editor from batch defaults");
  assert.match(formSource, /Per-grid Base Asset Qty|单格下单币数量/, "workspace form should explain base-asset sizing in clearer terms");
  assert.match(formSource, /lg:sticky|top-/, "workspace form should keep the preview column sticky on desktop");
  assert.match(formSource, /overall.*trigger.*before|整体止盈.*先触发/s, "workspace form should warn when overall take profit can preempt grid exits");
  assert.doesNotMatch(pageSource, /symbolMatches\[0\]\?\.symbol \?\? symbolQuery/, "new strategy page should not auto-select the first search result");
  assert.doesNotMatch(createRoute, /readField\(formData, "symbol"\) \|\| "BTCUSDT"/, "create route should not silently fall back to BTCUSDT");
  assert.match(createRoute, /search results|搜索结果/, "create route should surface an explicit symbol-selection error");
});

test("strategy preview is no longer an empty placeholder shell", () => {
  const previewSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-visual-preview.tsx");

  assert.match(previewSource, /svg|candles|kline|candle/i, "strategy preview should render a local candle chart instead of a remote-only embed");
  assert.match(previewSource, /grid line|gridLine|entryPrice/i, "strategy preview should overlay grid levels on the chart");
  assert.match(previewSource, /takeProfitPrice|TP Price|止盈价/i, "strategy preview should surface take-profit price previews");
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
