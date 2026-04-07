# Create Grid Strategy

Use `/app/strategies/new` to create a draft before the first launch.

## Draft Flow

1. Search the symbol with the fuzzy symbol search box.
2. Choose the market type and strategy mode.
3. Pick a generation mode:
   - Arithmetic for fixed percentage spacing around the reference price.
   - Geometric for compounded spacing around the reference price.
   - Fully custom when every grid will be edited manually.
4. Choose the editor mode:
   - Batch ladder builder for fast setup.
   - Custom JSON for every-grid overrides.

## Amount Mode

- Amount mode lets you size each grid by quote capital or base asset size.
- `Quote amount (USDT)` keeps every grid near the same USDT exposure.
- `Base asset quantity` keeps every grid at the same coin quantity.
- Batch mode converts the chosen amount mode into the real per-grid quantity saved in `levels_json`.

## Batch Ladder Controls

Batch mode uses these fields:

- `Reference price`
- `Grid count`
- `Batch spacing (%)`
- `Batch take profit (%)`
- `Trailing take profit (%)`

Use batch mode when you want a quick ladder. Switch to custom JSON when you need to set every grid entry price, quantity, take-profit range, and trailing rule separately.

## Profit And Risk Controls

- `Overall take profit (%)` closes the whole strategy when the total profit target is reached.
- `Overall stop loss (%)` is optional.
- `Trailing take profit (%)` is optional.
- When trailing take profit is enabled, the close is taker-style, so fees may be higher than maker-style limit take profit.
- Trailing take profit must not exceed the grid take-profit range.

## Templates

- `Apply template` copies the admin preset into your own draft.
- After copy, the draft belongs to you.
- Later template changes do not overwrite drafts you already created.

## Start Requirements

- Draft creation does not start the strategy.
- Pre-flight validates membership, exchange API posture, symbol metadata, balance or collateral, and futures hedge-mode requirements.
- Running strategy parameters cannot be hot-modified.
- Pause first, save changes, re-run pre-flight, then start or resume again.
