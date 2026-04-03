# Admin Guide: System Config and Audit

## Scope

Use `/admin/system` for chain confirmation policy review and `/admin/audit` for super-admin audit inspection. Treasury collection work from `/admin/sweeps` is part of the same operational control surface because sweep requests must be audited.

## Permissions

- `super_admin` can change confirmation counts, request sweeps, and review audit history.
- `operator_admin` can review `/admin/system` in read-only mode and is redirected away from `/admin/audit`.

## Confirmation Policy Workflow

1. Open `/admin/system` and review the stored confirmation counts for `ETH`, `BSC`, and `SOL`.
2. For a `super_admin` session, update the values and submit `Save confirmation policy`.
3. Confirm the success banner reflects the final counts.
4. For an `operator_admin` session, treat the page as read-only review; the inputs and save action remain disabled.

## Audit Expectations

Audit rows must capture:

- Actor
- Timestamp
- Action type
- Target entity
- Before and after summary when relevant
- Session metadata when relevant

## Critical Admin Controls

- Membership pricing and lifecycle changes
- Address pool inventory changes
- Template creation and template updates
- Treasury sweep requests
- System confirmation policy changes

## Sweep and Audit Review

- Submit treasury collection jobs from `/admin/sweeps` with the correct chain, asset, source address, and treasury destination.
- Verify sweep requests later appear in `/admin/audit` with action and before/after context.
- Use the audit view to confirm who changed sensitive commercial settings and when.
