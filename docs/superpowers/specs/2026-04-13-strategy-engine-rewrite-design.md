# Strategy Engine Rewrite Design

## 1. Purpose

This document replaces the mixed and partially incorrect strategy behavior currently present in the repository with a single explicit strategy model for grid construction, preview, runtime execution, and statistics.

The March 31 frozen product design remains the product scope baseline. This document narrows one specific subsystem: the strategy creation and runtime engine. It exists because the current implementation mixes multiple grid semantics, produces a preview that does not match runtime behavior, and treats ordinary single-sided grids and classic bilateral range grids as if they were the same product.

The goal of this rewrite is to make the following surfaces use one coherent set of rules:

- strategy creation form
- left-side strategy preview
- saved draft definition
- server-side pre-flight
- runtime order placement and replenishment
- strategy statistics and per-grid accounting

## 2. Rewrite Goal

Rebuild the strategy module as a state-machine-driven execution system with two clearly separated strategy types:

- `ordinary_grid`: the default and recommended mode
- `classic_bilateral_grid`: the optional classic range grid mode

Completion means all of the following are true:

- ordinary grids no longer build around the anchor price on both sides
- ordinary grids always start from the first level and then continue only on one side for replenishment
- take-profit orders are created only for levels that have actually filled
- classic bilateral grids remain available, but use their own runtime path and never reuse ordinary-grid first-fill behavior
- the preview panel is simplified and matches the real order-construction rules
- strategy controls such as `only_sell_no_buy` and `stop_after_take_profit` are enforced by runtime state instead of UI-only flags

## 3. Frozen Decisions For This Rewrite

### 3.1 Strategy Types And Product Naming

The product will expose two strategy types:

1. `ordinary_grid`
2. `classic_bilateral_grid`

`ordinary_grid` is the default choice in the create-strategy page.

For market-facing naming:

- spot ordinary grid is shown as the normal spot grid experience
- futures ordinary grid is shown as `long` or `short`
- classic bilateral grid is shown explicitly as a classic bilateral grid mode and is not the default

The product must stop presenting ordinary spot behavior as separate `spot buy-only` and `spot sell-only` strategy types during creation. The ordinary spot grid means: accumulate on the configured side and close each filled lot through its own take-profit order.

### 3.2 Ordinary Grid Core Semantics

Ordinary grid is single-sided for replenishment and per-level for exits.

For an ordinary grid:

- the first level price is the anchor price
- grid count includes the first level
- the first level immediately participates in real execution
- the remaining levels extend in one direction only
- no unfilled level may have a take-profit order in advance
- every filled level owns its own take-profit order and accounting record

Ordinary grid startup sequence:

- compute the full level price table
- immediately execute the first level by market order using the configured amount or quantity of that first level
- record the actual average fill price, quantity, fee, and fill identifier for level 1
- create level 1 take-profit order from the actual fill price of level 1
- place the remaining replenishment orders on the configured single side

Ordinary grid runtime sequence:

- when a replenishment level fills, create only that level's take-profit order
- when a level take-profit fills, mark that level cycle complete
- if the strategy remains in normal running state, replenish that level's entry order again according to the saved ordinary-grid rules
- if the strategy is draining, do not replenish a new entry order

### 3.3 Ordinary Grid Direction Rules

Ordinary grid direction is frozen as follows.

Spot ordinary grid and futures long:

- first level is the anchor price
- remaining replenishment levels extend downward only
- each filled level closes upward through its own take-profit sell order

Futures short:

- first level is the anchor price
- remaining replenishment levels extend upward only
- each filled level closes downward through its own take-profit buy order

The runtime must not create an ordinary grid that starts with both lower buy orders and upper sell orders at the same time.

### 3.4 Anchor Price Rules

For ordinary grid, the anchor price rules are frozen as follows.

If reference source is `market`:

- level 1 price is the latest market price captured at strategy start time
- level 1 participates in real execution immediately

If reference source is `manual`:

- level 1 price is the manually entered price
- level 1 participates in real execution immediately

This same rule applies to spot ordinary grid, futures long, and futures short.

### 3.5 Ordinary Grid Price Construction

Ordinary grid uses fixed-step percentage spacing derived from the first level price, not compounded recursion.

Definitions:

- `P1`: first-level anchor price
- `s`: spacing percentage expressed as a decimal
- `N`: grid count including level 1

For spot ordinary grid and futures long:

- `Pi = P1 × (1 - s × (i - 1))`

For futures short:

- `Pi = P1 × (1 + s × (i - 1))`

Example with `P1 = 70000` and `s = 1%`:

- long or spot ordinary: `70000, 69300, 68600, 67900, ...`
- short: `70000, 70700, 71400, 72100, ...`

The system must reject any ordinary-grid parameter set that makes a generated price non-positive or violates exchange tick and notional constraints after normalization.

### 3.6 Classic Bilateral Grid Rules

Classic bilateral grid remains available for both spot and futures, but it is a separate strategy type.

Classic bilateral grid rules:

- uses a center price instead of a first-level market-fill startup
- does not execute an immediate first market fill on start
- places bilateral range orders directly around the center price
- can use either fixed-step spacing or compounded geometric spacing, chosen by the user
- for futures accounts, hedge mode is mandatory
- pre-flight must fail if the futures account is not in hedge mode and must tell the user exactly that this step failed

Classic bilateral grid must not reuse ordinary-grid state transitions, order sequencing, or preview rules.

### 3.7 Per-Level Take Profit Rules

Each ordinary-grid level owns its own take-profit logic.

Take-profit price is always derived from the level's actual fill price, never from the anchor price.

For spot ordinary grid and futures long:

- `TPi = FillPrice_i × (1 + tp_i)`

For futures short:

- `TPi = FillPrice_i × (1 - tp_i)`

Level 1 is not special in accounting. It is a normal level and must be tracked exactly like the rest.

### 3.8 Trailing Take Profit Rules

The previously frozen trailing take-profit product rule remains in force and is clarified here for the rewritten engine:

- trailing take profit is optional and configured per level
- trailing take profit is allowed only when its trailing percentage is less than or equal to that level's take-profit percentage
- when trailing take profit is absent, that level closes through a maker-style limit take-profit order
- when trailing take profit is present, that level closes through taker-style market exit after the trigger activates and the trailing drawdown is hit
- the runtime must track the post-trigger extreme price for that level, not the anchor price and not the pre-trigger price history
- the UI and pre-flight must warn that trailing take profit uses taker execution and may increase fees

### 3.9 Runtime Control: Only Sell No Buy

A running strategy can be switched into `only_sell_no_buy` mode. This label stays literal for all markets and directions even though the runtime meaning is direction-aware.

When `only_sell_no_buy` is enabled:

- all unfilled replenishment orders are canceled immediately
- all existing take-profit orders are kept
- no new replenishment orders may be placed
- no new replenishment cycles may begin
- the strategy transitions into `draining`
- the runtime waits until existing inventory or position is fully closed through the remaining take-profit orders or a forced strategy stop

Because all replenishment orders are removed, no new filled entry levels can appear after the mode is enabled. Therefore no new take-profit orders are created after the transition into draining.

### 3.10 Runtime Control: Stop After Take Profit

A strategy can enable `stop_after_take_profit`.

This flag stops the strategy when either of the following occurs:

- overall take profit triggers
- the strategy is in `only_sell_no_buy` mode and all remaining position has been closed

This flag does not stop the strategy after a normal single-level take-profit fill while the grid is still in its regular replenishment cycle.

### 3.11 Overall Take Profit And Overall Stop Loss

The ordinary-grid runtime keeps the existing product concept of overall take profit and optional overall stop loss.

Overall take profit and overall stop loss operate on strategy-level active exposure, using the same strategy-level realized and unrealized accounting surfaces already required by the frozen March 31 design.

If the configured overall take profit is so low that it is likely to trigger before ordinary per-level take-profit behavior becomes meaningful, the UI must show a strong warning, but saving is still allowed.

## 4. Runtime Architecture

### 4.1 Single Strategy Definition

All strategy creation, persistence, preview, and execution must consume a single normalized server-side strategy definition.

The definition must include at minimum:

- strategy type
- market type
- direction
- reference source
- anchor price or center price semantics
- spacing algorithm
- grid count or per-side grid count
- amount mode
- per-level amount configuration
- per-level take-profit configuration
- per-level trailing configuration
- overall take profit
- overall stop loss
- post-take-profit stop behavior

The browser must stop acting as an independent grid-construction engine.

### 4.2 Builder And Runtime Split

The engine is split into these layers:

1. `StrategyDefinitionNormalizer`
2. `GridBuilder`
3. `RuntimeStateMachine`
4. `OrderPlanner`
5. `StatisticsProjector`

`StrategyDefinitionNormalizer`
- validates and canonicalizes user inputs
- converts user-facing form data into an internal immutable strategy definition

`GridBuilder`
- constructs ordinary-grid or classic-bilateral logical levels from the normalized definition
- is the only source of truth for price ladder construction
- is used by both preview and runtime

`RuntimeStateMachine`
- owns lifecycle state transitions
- keeps ordinary-grid and classic-bilateral runtimes separate
- enforces draining and stop-after-take-profit behavior

`OrderPlanner`
- converts runtime state into concrete exchange orders
- is the only place where order intent becomes an exchange request

`StatisticsProjector`
- derives per-level and per-strategy statistics from persisted events and fills

### 4.3 Runtime States

The strategy runtime state machine is frozen to the following top-level states:

- `draft`
- `preflight_ready`
- `starting`
- `running`
- `draining`
- `stopped`
- `error`

Ordinary-grid state transitions:

- `draft -> preflight_ready -> starting -> running`
- `running -> draining` when `only_sell_no_buy` is enabled
- `running -> stopped` when overall take profit with stop flag closes the strategy
- `draining -> stopped` when remaining position is fully closed and stop flag is enabled
- `running|draining -> error` when reconciliation or exchange execution integrity breaks

Classic bilateral grid uses the same top-level lifecycle states, but its internal runtime rules are separate.

### 4.4 Runtime Event Model

The event stream must explicitly represent the rewritten lifecycle.

Required event categories:

- first-level market fill executed
- ordinary replenishment order placed
- ordinary replenishment level filled
- level take-profit order placed
- level take-profit filled
- ordinary replenishment cycle resumed
- draining mode entered
- overall take profit triggered
- overall stop loss triggered
- strategy auto-stopped after take-profit rule
- classic bilateral bid order placed
- classic bilateral ask order placed
- pre-flight hedge-mode failure

These events drive the user strategy timeline and statistics.

## 5. UI And Preview Design Boundaries

### 5.1 Strategy Creation Layout

The create-strategy page must branch by strategy type early.

The first decision block is:

- strategy type: `ordinary_grid` or `classic_bilateral_grid`
- market: `spot` or `futures`
- futures direction where applicable: `long`, `short`, or classic bilateral futures

Ordinary-grid form shows only fields relevant to ordinary single-sided runtime.

Classic bilateral form shows only fields relevant to bilateral range runtime.

The page must stop presenting fields that make bilateral assumptions while the user is configuring an ordinary single-sided strategy.

### 5.2 Preview Panel

The left preview chart must be simplified.

For ordinary grid preview, show only:

- anchor price or current price line
- grid level lines
- overall covered range shading

Do not show:

- take-profit lines
- crowded per-level text labels inside the chart
- bilateral structures when the user is configuring an ordinary grid

For classic bilateral preview, show only:

- center line
- upper and lower grid range
- bilateral ladder lines

The preview panel must consume builder output. It may not compute a separate ladder in the client that differs from runtime semantics.

### 5.3 Ordinary-Grid Preview Behavior

When the user selects `market` reference source, the preview anchor updates from the live market preview feed and rebuilds the ordinary ladder from that live anchor.

When the user selects `manual`, the preview anchor uses the manual price directly.

The preview must make it obvious that ordinary grid is one-sided for replenishment.

### 5.4 Runtime Controls In User UI

The strategy detail surface must expose these runtime controls clearly:

- `only_sell_no_buy`
- `stop_after_take_profit`

Enabling `only_sell_no_buy` requires an explicit confirmation dialog that states:

- unfilled replenishment orders will be canceled
- no new replenishment orders will be placed
- only existing position exits will remain active

The UI label remains literally `只卖不买` for all markets and directions.

## 6. Persistence And Data Model Changes

The rewrite requires persistence that distinguishes logical grid definitions from executed lots.

The accepted data model must support at minimum:

- strategy definition revision with explicit `strategy_type`
- ordinary-grid logical levels
- classic bilateral logical levels or range segments
- executed entry lots per level
- per-lot take-profit configuration and status
- runtime control flag history
- strategy transition history
- per-level realized PnL, fee, and close status

The current persistence shape that stores only loose level arrays is insufficient as the sole accepted model for runtime truth.

The rewrite may keep compatibility reads for old drafts during migration, but newly saved strategies must persist enough structure for the new runtime without reconstructing missing semantics from UI conventions.

## 7. Statistics Rules

### 7.1 Per-Level Accounting

For ordinary grid, every filled level is a first-class accounting unit.

Each level must track:

- intended entry price
- actual fill price
- actual fill quantity
- take-profit target percentage
- take-profit execution mode
- take-profit fill price
- realized PnL
- paid fee
- runtime cycle state

Level 1 is included in this accounting exactly like every later filled level.

### 7.2 Strategy-Level Accounting

Strategy totals must aggregate:

- realized PnL across all levels
- unrealized PnL for open inventory or position
- total fees
- total funding where applicable
- closed-level count and open-level count

Ordinary-grid and classic-bilateral statistics must be identifiable by strategy type so that reporting never mixes their semantics.

## 8. Migration Boundaries

This rewrite is allowed to invalidate or migrate current strategy drafts whose behavior was built from incorrect ordinary-grid semantics.

Migration rules:

- existing drafts saved under the current mixed model may be reopened only after normalization into the new strategy definition
- any draft that cannot be normalized safely must be marked incompatible and require user review before restart
- currently running strategies must not be silently reinterpreted; they need explicit rebuild or restart handling under the new runtime version

## 9. Acceptance Criteria

This rewrite is complete only when all of the following are true.

### 9.1 Ordinary Grid Acceptance

- start uses an immediate first market fill
- remaining replenishment orders are placed only on one side
- no unfilled level has a pre-created take-profit order
- each filled level gets its own take-profit order from its actual fill price
- `only_sell_no_buy` cancels all remaining replenishment orders and enters draining
- `stop_after_take_profit` stops after overall take profit or after draining reaches full close

### 9.2 Classic Bilateral Acceptance

- no first-level immediate market fill is used
- bilateral orders are placed around a center price
- user can choose fixed-step or geometric spacing
- futures classic bilateral fails pre-flight when hedge mode is off

### 9.3 Preview Acceptance

- ordinary-grid preview is visibly one-sided
- classic-bilateral preview is visibly bilateral
- the preview chart shows anchor or center, level lines, and covered range only
- take-profit lines are absent from the chart
- saved strategy definitions and preview ladders match runtime builder output

### 9.4 Statistics Acceptance

- first-level market fill appears in per-level and strategy-level accounting
- each filled level exposes its realized PnL and fees
- strategy totals match the sum of level outcomes plus open exposure

### 9.5 Verification Requirements

The implementation plan and later execution must include failing tests first for:

- ordinary-grid builder formulas
- classic-bilateral builder formulas
- first-level immediate market-fill startup behavior
- per-level take-profit creation only after real fills
- draining behavior after `only_sell_no_buy`
- stop-after-take-profit behavior
- futures classic bilateral hedge-mode pre-flight failure
- preview contract for one-sided vs bilateral ladder rendering
- statistics projection for first-level and later-level fills

## 10. External Pattern Reference

This rewrite intentionally separates two industry-visible patterns that were previously conflated in the codebase:

- classic bilateral range grid behavior, as commonly described by public grid-bot platforms such as 3Commas and Pionex
- the product-default ordinary grid behavior confirmed in this design, where the first level fills immediately and later levels replenish only on one side while each filled level closes through its own take-profit order

The implementation must follow the product-specific ordinary-grid rule confirmed in this design even where public classic-grid products use a different default behavior.
