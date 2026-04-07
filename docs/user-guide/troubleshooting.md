# Troubleshooting

## Login And Security

- If login says `admin totp setup required`, open `/admin-bootstrap`, create the admin TOTP secret, then return to `/login` and enter the shown TOTP code.
- If normal user login says `totp code required`, enter the current code from your authenticator app.

## Exchange Problems

- If the platform shows `API credentials invalid`, open `/app/exchange` and inspect the validation details.
- Common failure reasons include timestamp drift, missing permissions, withdrawal permission still enabled, market access mismatch, or hedge mode mismatch.

## Strategy Problems

- If start fails, read the failed pre-flight step in `/app/strategies/:id`.
- If stop remains in `Stopping`, the platform is still reconciling exchange order cancellation or market-close execution.
- If delete is blocked, clear remaining positions and working orders first.

## Billing Problems

- Payment amounts must match exactly.
- Wrong token, wrong amount, or unmatched transfers go to manual review.
- If a payment stays in `confirming`, wait for the required chain confirmations.
