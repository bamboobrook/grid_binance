# Membership And Payment

Membership is required before a strategy can start.

## Plans

Default starting prices are:

- Monthly: 20 USD equivalent
- Quarterly: 18 USD equivalent per month
- Yearly: 15 USD equivalent per month

Admins can change prices and durations for later renewals.

## Payment Rules

- Supported chains: Ethereum, BSC, Solana
- Supported stablecoins: USDT, USDC
- The amount must match exactly.
- Overpayment, underpayment, wrong token, or abnormal transfers go to manual review.

## Address Assignment

When you create a payment order, the billing page shows one of these states:

- `Assigned address`: send the exact amount to that address only.
- `Queue position`: all addresses on that chain are busy, so your order is waiting for the next free address.
- `Address lock expires`: the assigned address stays reserved for your order during the lock window.

Always check:

- Assigned address
- Chain and token
- Exact amount
- Address lock expiry time

## Grace Period

- Membership expiry enters a 48-hour grace period.
- Existing running strategies may continue only during that window.
- New starts are blocked after expiry.
- After grace ends, running strategies are auto-paused.

## Manual Review

If your transfer is abnormal:

- Wait for the deposit to appear in the admin review queue.
- Do not send a second transfer until the first one is reviewed.
- Keep the tx hash and wallet screenshot ready in case support asks for it.
