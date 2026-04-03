# Telegram Notifications

Bind Telegram once and keep runtime, billing, and API alerts mirrored between web and chat.

Open `/app/telegram` after sign-in. This is the canonical app route for Telegram binding and delivery status.

Bind a short-lived code in the web app at `/app/telegram`, then send it to the Telegram bot to complete linking.

A single user can bind only one Telegram identity, and the issued bind code should expire quickly after creation.

Telegram mirrors strategy lifecycle, API credential issues, membership reminders, deposit confirmations, and per-fill profit summaries.
