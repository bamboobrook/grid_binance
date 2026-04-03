# Security Center

## Scope

Use `/app/security` to manage password and TOTP settings without exposing exchange secrets.

## What The Security Center Covers

- password updates
- TOTP enablement and verification
- account posture review
- masked Binance credential status

## Security Rules

- Email verification must complete before normal login.
- Use a unique password for this platform.
- TOTP is supported for users and administrators.
- Admin accounts are expected to use TOTP.
- Password changes and TOTP actions must use authenticated POST flows.
- Binance API secrets remain encrypted at rest and masked after save.
- Withdrawal permission must remain disabled on Binance API keys.

## Recommended User Workflow

1. Sign in at `/login`.
2. Open `/app/security`.
3. Enable TOTP before binding live Binance credentials.
4. Store the recovery material for your authenticator app outside this repository.
5. Re-check that Binance credentials remain masked after save.

## When To Revisit This Page

- after a password reset
- after device trust changes
- before switching Binance credentials
- before going live with futures strategies
