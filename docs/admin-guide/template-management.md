# Admin Guide: Template Management

## Scope

Use `/admin/templates` to maintain admin-owned strategy templates that users can apply into their own draft configurations. In the current product, this page is a `super_admin` control surface.

## Permissions

- `super_admin` can create, edit, and maintain template definitions.
- `operator_admin` does not have a template review surface in the current admin app and should not use this page as part of normal operations.

## Super Admin Workflow

1. Open `/admin/templates` and review the current inventory.
2. Create or edit a template with the fields the page currently exposes: template name, symbol, market, mode, generation, two grid levels, readiness booleans, overall take-profit, optional overall stop-loss, and post-trigger action.
3. Save the template and verify it appears in the inventory table.
4. Use the inventory `Edit` action to load an existing template back into the form when a future revision is needed.

## Product Rules

- Templates are copied into user-owned strategies when applied.
- Later template updates do not change already-applied user strategies.
- Users can still modify their template-derived drafts after application.

## Current Surface Reference

- Inventory columns currently shown: `Name`, `Symbol`, `Market`, `Generation`, and `Levels`.
- `super_admin` sessions also get an `Actions` column with the `Edit` action.
- The current page does not expose a separate operator review mode, approval queue, readiness summary table, or template actions beyond create and edit.
- The inventory table does not list readiness flags, risk annotations, overall TP/SL values, or post-trigger action inline; those fields are only available inside the create/edit form for `super_admin`.
