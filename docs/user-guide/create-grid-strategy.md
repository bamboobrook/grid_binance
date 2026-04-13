# Create Grid Strategy

Use `/app/strategies/new` to create a draft before the first launch.

## Draft Flow

1. Search the symbol with the fuzzy symbol search box and click a result to lock it.
2. Choose the market and grid family.
3. Pick the generation mode:
   - Arithmetic for fixed-percentage spacing.
   - Geometric for compounded spacing.
   - Fully custom when every grid is edited manually.
4. Pick the editor mode:
   - Batch builder for fast ladder generation.
   - Per-grid custom when you want to edit every level in the GUI.

## Strategy Families

- `Ordinary grid` is the default product path.
  - Startup immediately market-fills level 1.
  - Later replenishment orders stay on one side only.
  - Every filled level becomes its own accounting and take-profit unit.
  - A take-profit order is created only after that level has actually filled.
- `Classic bilateral grid` is a separate bilateral range strategy.
  - It does not use the level-1 startup market fill.
  - It places orders on both sides around the center price.
  - It follows its own bilateral range rules and should be chosen explicitly.

## Reference Price

- `Reference source` supports either manual price or current market price.
- Manual mode uses the exact number you enter.
- Current-price mode lets the page preview the latest market price before start.
- For market-reference strategies, level 1 / the anchor price is captured when the strategy starts.
- The page preview can show the latest price in advance, but live execution uses the start-time anchor instead of an earlier draft-time quote.
- When you use per-grid custom mode, the reference price becomes a helper for preview and row creation instead of overriding your manual rows.

## Amount And Level Editing

- `Quote amount (USDT)` keeps each grid near the same quote exposure.
- `Base quantity` keeps each grid at the same asset quantity.
- Per-grid custom mode lets you edit each row directly:
  - entry price
  - spacing versus the previous level
  - per-grid amount
  - grid take profit range
  - optional trailing take profit
- The page still serializes these rows into `levels_json` internally, but users no longer need to hand-edit JSON for normal usage.

## Batch Ladder Controls

Batch mode uses these fields:

- `Reference price`
- `Grid count`
- `Batch spacing (%)`
- `Grid take profit (%)`
- `Trailing take profit (%)`

Use batch mode when you want a quick ladder. Switch to per-grid custom when you need to edit each grid independently.

## Profit And Risk Controls

- `Overall take profit (%)` closes the whole strategy when the total active exposure reaches the configured profit target.
- `Overall stop loss (%)` is optional. Leave it empty to disable it.
- `Trailing take profit (%)` is optional.
- When trailing take profit is enabled, the close is taker-style, so fees may be higher than maker-style limit take profit.
- Trailing take profit must not exceed the corresponding grid take-profit range.

## Runtime Controls After Start

- `Only Sell No Buy` cancels all remaining replenishment orders and leaves only the existing exits / close orders working.
- `Stop after take profit` stops the strategy after an overall take-profit trigger, or after an `Only Sell No Buy` drain fully closes the remaining exposure.
- For ordinary grid, level 1 stays in the same per-level statistics as every later filled level.

## Templates

- `Apply template` copies the admin preset into your own draft.
- After copy, the draft belongs to you.
- Later template changes do not overwrite drafts you already created.

## Start Requirements

- Draft creation does not start the strategy.
- Pre-flight validates membership, exchange API posture, symbol metadata, spot balance or futures collateral, and hedge-mode requirements where applicable.
- Running strategy parameters cannot be hot-modified.
- Pause first, save changes, re-run pre-flight, then start or resume again.
