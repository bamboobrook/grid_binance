# Admin Guide: Membership Operations

## Scope

Use `/admin/memberships` for plan pricing and membership lifecycle operations.

## Permissions

- `super_admin` can manage plan pricing and membership state.
- `operator_admin` can review current pricing and membership rows, but cannot persist changes.

## Pricing Management

- Prices are configured per chain and token pair.
- The current UI exposes the supported matrix for `ETH`, `BSC`, `SOL` with `USDT` and `USDC`.
- Updated pricing affects new purchase orders and future renewals; it does not rewrite active entitlements.

## Membership Lifecycle Actions

1. Use `Open membership` to create or reopen an entitlement for a user.
2. Use row actions to `Extend membership`, `Freeze membership`, `Unfreeze membership`, or `Revoke membership`.
3. Confirm the success banner shows the target email, resulting status, and last action.
4. Review the membership table to verify the backend snapshot reflects the change.

## Rules To Remember

- Memberships can be activated automatically by successful payment or manually by admin action.
- Renewals stack on top of the current expiry.
- Grace period lasts 48 hours after expiry.
- Freeze and revoke are explicit admin overrides.

## Operational Checks

- Confirm the plan pricing table matches the intended commercial offer before sending users to pay.
- Verify the target email carefully before applying lifecycle actions.
- Expect operator sessions to see the screen in read-only mode for pricing and lifecycle mutations.
