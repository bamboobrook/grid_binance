# Create Grid Strategy

Review draft creation, pre-flight validation, and start requirements before your first launch.

Open `/app/strategies/new` to create a draft. This is the canonical app route for the new-strategy composer.

Drafts can be edited freely until you run pre-flight and start the strategy.

After the draft is created, continue from the strategy detail workspace under `/app/strategies/:id` for save, pre-flight, and start actions.

Pre-flight validates exchange filters, available balance, and hedge mode before a strategy can start.

Running strategy parameters cannot be hot-modified. Pause, save edits, and re-run pre-flight before restart.

Trailing take profit uses taker execution and may increase fees compared with maker-style take-profit orders.
