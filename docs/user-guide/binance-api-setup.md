# Binance API Setup

Create and bind Binance API credentials safely before running any strategy.

Open `/app/exchange` after sign-in. This is the canonical app route for Binance credential save, masking, and connection verification.

Create a Binance API key with trading and read permissions only. Withdrawal permission must remain disabled.

Save the API key and secret through the exchange credentials form at `/app/exchange`, which stores the secret server-side and only shows the masked key back to the user.

Run the connection test on `/app/exchange` after saving to confirm Spot, USDⓈ-M, COIN-M scopes and the required hedge mode posture for futures strategies.

## Live Test Notes

- The platform does not read Binance API keys from the server `.env` file. Each user must save their own key in `/app/exchange`.
- If Binance key permissions are restricted by IP, use the public outbound IP of the machine running `api-server` and `scheduler`.
- For a first live pass, start with Spot only and a very small amount. Add futures permissions and Hedge Mode only when you are ready to test futures-specific flows.
- After saving the key, always run the exchange connection test before creating or restarting a strategy.
