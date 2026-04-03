# Admin Guide: Address Pool Management

## Scope

Use `/admin/address-pools` to manage the billing address inventory for `ETH`, `BSC`, and `SOL`.

## Permissions

- `super_admin` can add, enable, and disable addresses.
- `operator_admin` can review inventory only.

## Operating Rules

- Pools are managed per chain.
- Each order receives one dedicated address during the 1-hour lock window.
- Address assignment rotates through enabled inventory.
- When all eligible addresses are locked, new orders wait in queue until an address is released.

## Routine Workflow

1. Open `/admin/address-pools` and review the `Enabled inventory` summary.
2. Confirm the chain inventory still has enough enabled addresses for new orders.
3. To expand capacity, choose the chain, enter the new address, and submit `Add or enable address`.
4. To remove an address from new assignment, use the row action to disable it.
5. Re-check the inventory table and success banner to confirm the backend state changed.

## Day-2 Notes

- Add addresses before a pool runs out; queueing is expected only when enabled capacity is temporarily exhausted.
- Address pool management does not replace treasury collection. Stablecoin collection is handled from `/admin/sweeps`.
- Inventory changes are restricted because pool availability directly affects membership payment order fulfillment.
