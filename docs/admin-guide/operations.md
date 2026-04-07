# Admin Operations

## Admin Login

- Configured admin accounts must complete TOTP before they can access the admin control plane.
- First-time admin setup uses `/admin-bootstrap`.
- After bootstrap, admins log in from `/login` with email, password, and the current TOTP code.
- `operator_admin` and `super_admin` still follow the same TOTP requirement.

## Admin Routes

- Shared admin navigation: `/admin/dashboard`, `/admin/users`, `/admin/memberships`, `/admin/deposits`, `/admin/address-pools`, `/admin/strategies`, `/admin/sweeps`, `/admin/system`.
- `super_admin` additionally uses `/admin/templates` and `/admin/audit`.

## Runtime Checks

- Use `docker compose --env-file .env -f deploy/docker/docker-compose.yml ps` to verify service status.
- Review `/admin/deposits` for abnormal transfer cases.
- Review `/admin/address-pools` when pool pressure or queue buildup appears.
- Review `/admin/system` for chain confirmation policy changes.

## Operational Notes

- Membership payments only auto-apply after the required chain confirmations arrive.
- API invalidation notices now originate from real exchange validation failures, not only manual test dispatches.
- Strategies with open positions may remain in `Stopping` until exchange close reconciliation completes.
