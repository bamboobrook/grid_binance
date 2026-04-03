# Manage Strategy

Understand the pause, edit, pre-flight, start, and reporting lifecycle for an existing strategy.

Open `/app/strategies/:id` to manage one strategy. This is the canonical app route for the strategy detail workspace.

Use `/app/strategies` to return to the strategy list, then open the target detail page under `/app/strategies/:id`.

Pause before editing, save the changes, and re-run pre-flight before restart.

Strategy-level statistics must show realized PnL, unrealized PnL, fees, funding fees, net profit, cost basis, fill count, order count, and current holdings.

Delete is allowed only when working orders and positions have both been cleared.
