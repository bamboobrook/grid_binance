# Binance API Setup

Create and bind Binance API credentials safely before running any strategy.

Open `/app/exchange` after sign-in. This is the canonical app route for Binance credential save, masking, and connection verification.

Create a Binance API key with trading and read permissions only. Withdrawal permission must remain disabled.

Save the API key and secret through the exchange credentials form at `/app/exchange`, which stores the secret server-side and only shows the masked key back to the user.

Run the connection test on `/app/exchange` after saving to confirm Spot, USDⓈ-M, COIN-M scopes and the required hedge mode posture for futures strategies.
