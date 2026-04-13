# Create Grid Strategy

Use `/app/strategies/new` to create a draft before the first launch.

## Draft Flow

1. Search the symbol with the fuzzy symbol search box and click a result to lock it.
2. Choose the market type and strategy mode.
3. Pick the generation mode:
   - Arithmetic for fixed-percentage spacing.
   - Geometric for compounded spacing.
   - Fully custom when every grid is edited manually.
4. Pick the editor mode:
   - Batch builder for fast ladder generation.
   - Per-grid custom when you want to edit every level in the GUI.

## Reference Price

- `Reference source` supports either manual price or current market price.
- Manual mode uses the exact number you enter.
- Current-price mode pulls the latest Binance ticker price before saving the draft.
- When you use per-grid custom mode, the reference price becomes a helper for preview and row creation instead of overriding your manual rows.

## Amount And Level Editing

- `Quote amount (USDT)` keeps each grid near the same quote exposure.
- `Base quantity` keeps each grid at the same asset quantity.
- Spot classic and spot sell-only also need enough base-asset inventory for sell-side grids during pre-flight.
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

## Templates

- `Apply template` copies the admin preset into your own draft.
- After copy, the draft belongs to you.
- Later template changes do not overwrite drafts you already created.

## Start Requirements

- Draft creation does not start the strategy.
- Pre-flight validates membership, exchange API posture, symbol metadata, balance or collateral, and futures hedge-mode requirements.
- Running strategy parameters cannot be hot-modified.
- Pause first, save changes, re-run pre-flight, then start or resume again.
