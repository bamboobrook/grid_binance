# Manage Strategy

## Main Pages

- `/app/strategies` shows drafts, running strategies, paused strategies, and filtered batch actions.
- `/app/strategies/:id` shows one strategy workspace with pre-flight status, runtime events, and edit controls.
- `/app/orders` shows fills, exchange trade history, and order exports.
- `/app/analytics` shows account-level and strategy-level PnL, fees, funding, and wallet snapshots.

## Lifecycle Rules

- Create starts as `Draft`.
- Start runs pre-flight first. If any required step fails, start is blocked.
- Pause cancels working orders and keeps holdings or futures positions.
- Resume reruns pre-flight and rebuilds the strategy from the saved configuration plus current holdings context.
- If you stop a strategy with no remaining holdings or positions, it moves directly to `Stopped`.
- If you stop a strategy with remaining holdings or positions, it enters `Stopping` first. The engine cancels exchange orders, submits market close orders, and only moves to `Stopped` after exchange close reconciliation completes.
- Delete is blocked until working orders and remaining positions are both cleared.

## Edit Rules

- Running strategies cannot be hot-modified.
- Pause, save, re-run pre-flight, then restart.
- Batch actions support `Start selected`, `Pause selected`, `Delete selected`, filtered batch actions, and `Stop all`.

## Failure Guidance

- If pre-flight fails, the page shows the exact failed step.
- If exchange credentials become invalid, the platform raises an in-app and Telegram API invalidation notification with the failing validation reason.
- If runtime reconciliation fails, the strategy auto-pauses and records a runtime event with remediation guidance.
