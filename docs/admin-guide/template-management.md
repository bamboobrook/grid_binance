# Admin Guide: Template Management

## Scope

Use `/admin/templates` to maintain admin-owned strategy templates that users can apply into their own draft configurations.

## Permissions

- `super_admin` can create, edit, and maintain template definitions.
- `operator_admin` can open `/admin/templates` and review template inventory only.

## Super Admin Workflow

1. Open `/admin/templates` and review the current inventory.
2. Create a template with symbol, market, mode, generation, and at least the required grid levels.
3. Set readiness fields to reflect whether the template is safe for membership users.
4. Configure overall take-profit, optional stop-loss, and post-trigger action.
5. Save the template and verify it appears in the inventory table.
6. Use the `Edit` action when a future template revision is needed.

## Operator Review Workflow

1. Open `/admin/templates` and review the current inventory.
2. Confirm the symbol, market, mode, readiness, and risk fields are complete.
3. Escalate any required template create or edit work to a `super_admin`.

## Product Rules

- Templates are copied into user-owned strategies when applied.
- Later template updates do not change already-applied user strategies.
- Users can still modify their template-derived drafts after application.

## Review Checklist

- Confirm symbol and market match the intended exchange product.
- Confirm level entry prices, quantities, take-profit values, and optional trailing values are complete.
- Confirm readiness flags are honest; they are part of the operator review surface.
- Expect operator sessions to show the page without create or edit controls, and without any save path.
