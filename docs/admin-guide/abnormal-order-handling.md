# Admin Guide: Abnormal Order Handling

## Scope

Use `/admin/deposits` for payment exceptions that cannot be auto-matched into a normal membership activation flow.

## Permissions

- `operator_admin` can review and process abnormal deposits.
- `super_admin` can perform the same review actions.

## What Lands Here

Abnormal handling is required for overpayment, underpayment, wrong token, or other transfers that fail exact-match validation.

## Review Workflow

1. Open `/admin/deposits` and inspect the `Deposit exception queue`.
2. Use the tx hash, chain, and reason fields to identify the case.
3. If the transfer should not activate service, choose `Reject`.
4. If the transfer should be applied to a linked membership order, choose `Credit ... to membership`.
5. Confirm the success banner reports the resulting decision and tx hash.
6. Re-check the queue and the `Manual credit target order` panel to confirm the backend state updated.

## Route Notes

- `/admin/billing` is retained only as a legacy route and redirects to `/admin/deposits`.
- Operators should use `/admin/deposits` in runbooks and bookmarks.

## Decision Discipline

- Only credit when the linked order and payment context are clear.
- Reject ambiguous or invalid transfers rather than forcing a membership grant.
- Keep in mind that abnormal-order processing is part of the commercial control boundary exposed to operator admins.
