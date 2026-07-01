# 2026-07-01 External Martingale/Grid Claim Gate Matrix

> Objective checked: find any public martingale/grid/DGT candidate that can plausibly satisfy `<5000U`, multi-symbol, anti-overfit segment balance, conservative `ann >50% / DD<=10%`, balanced `ann >90% / DD<=20%`, aggressive `ann >110% / DD<=30%`, and current Binance-live reproducibility.
>
> Safety: research-only. This report does not trade, touch Binance, flyingkid, live mode, or real funds.

## Result

No public external claim found in this check satisfies the original gates.

The external evidence is consistent with the internal frontier:

- High martingale/DCA returns are paired with drawdowns far above the allowed gates.
- Lower-risk DCA/grid settings either still exceed the conservative DD gate or do not disclose enough evidence.
- DGT-style academic evidence improves traditional grid behavior, but published figures remain below the balanced/aggressive return gates or above the DD gates.
- Vendor bot pages and backtest tools generally expose APY/APR and drawdown fields, but do not provide trade-level, multi-period, multi-symbol, live-parity evidence.

## 2026-07-01 Latest Web Refresh

The latest web refresh did not overturn the matrix. Newer or recently updated public pages still fail at least one mandatory gate before they can become candidates:

- Pionex's current DCA/Martingale help page repeats the BTC/USDT April 2020 to July 2021 example: `205.68% APR` with `-52.84%` max drawdown, plus a more conservative setting with `122.12%` profit and `-16.37%` max drawdown. These remain single-symbol, old-window examples and fail the conservative DD gate or all DD gates.
- Phemex's Q1 2026 bot report lists ETH futures DCA/Martingale closed-bot ROI around `18-35%` and SOL spot DCA around `22-40%` for Q1 2026. It explicitly flags futures DCA/Martingale as the highest-drawdown bot type. The report is quarter-specific and does not provide the required 2023-2026 segment proof, capital proof, or live-parity replay.
- OKX's Futures DCA/Martingale help page, updated 2026-05-29, confirms the mechanism uses futures Martingale/DCA logic, relies on additional orders after adverse moves, and carries liquidation risk when margin falls below maintenance requirements. It is product documentation, not a qualifying audited portfolio.
- 3Commas currently documents multi-pair DCA bots, historical backtesting, trailing/breakeven stop loss, and futures hedge-mode support, but does not publish a specific C/B/A candidate with trade list, segment balance, drawdown, budget, and current Binance-live parity.
- Bitsgap's 2026 grid explanation still describes grid as sideways/volatile-market logic and DCA as downtrend/accumulation logic; it also notes grid risk when price breaks the range and DCA risk from larger lower-price positions. This is risk context, not a qualifying candidate.
- Neutralis's 2026 market-making article cites the same DGT paper with about `60-70%` annualized IRR. That level can be relevant to the conservative return target but still does not satisfy balanced/aggressive targets and lacks the requested 2025/2026_ytd segment/live proof.

## Gate Matrix

| Source | Public claim | Gate result | Why it does not qualify |
|---|---:|---|---|
| Pionex DCA/Martingale Simple Mode help page | BTC/USDT backtest from 2020-04-01 to 2021-07-01; default AI parameters report `205.68% APR` and `-52.84%` max drawdown | Reject | Annualized return is high, but DD fails all C/B/A gates, it is single-symbol BTC/USDT, wrong period, no 2023H1/H2/2024/2025/2026_ytd segment proof, no `<5000U` capital proof, and not Binance USD-M live-parity evidence. |
| Pionex conservative DCA/Martingale example | More conservative 10% drop scale reports `122.12%` profit and `-16.37%` max drawdown | Reject | DD fails conservative `<=10%`; the source does not clearly state this as annualized return, and it still lacks required multi-symbol, period-segment, capital, and Binance-live evidence. |
| Dynamic Grid Trading arXiv 2506.11921 | BTC/ETH 1m backtest from 2021-01 to 2024-07; DGT reaches roughly `60-70%` IRR in favorable parameter regions | Reject | This can be relevant to conservative return, but not balanced/aggressive targets; the paper notes ETH DGT drawdown around `50%` when the market declined around `80%`, which fails the DD gates. It also lacks the requested 2025/2026_ytd segments and live-execution parity. |
| 3Commas DCA/backtest documentation | Platform supports DCA bot backtesting and exposes APY, minimum deposit, max floating drawdown, and performance charts | Not evidence | This describes tooling, not a public candidate satisfying the objective. No complete parameter set, trade list, segment proof, or current Binance USD-M live replay is provided. |
| Bitsgap grid/DCA/backtest documentation | Platform supports grid/DCA backtests; Bitsgap warns that deep drawdowns such as `30-40%` are high-risk for live trading | Not evidence | This is useful risk context, but not a qualifying martingale/grid portfolio. It does not disclose a C/B/A candidate with the required periods and capital/live constraints. |
| Phemex bot strategy report | States Futures DCA/Martingale bots carry the highest drawdown risk among Phemex bot types | Reject as candidate | Risk note supports the internal diagnosis; it does not provide a qualifying low-DD, high-return, multi-period candidate. |

## Live-Parity Constraints From Current Binance Docs

Any external claim would still need a separate live-parity implementation check before becoming a candidate:

- USD-M Futures symbol rules must come from `GET /fapi/v1/exchangeInfo`, including lot size, market lot size, order limits, percent-price, and min-notional filters.
- Binance USD-M conditional TP/SL and trailing-stop orders now use the Algo Order endpoint `POST /fapi/v1/algoOrder`.
- Binance's derivatives change log says USD-M conditional orders migrated to Algo Service effective 2025-12-09; old stop-order assumptions are insufficient. It also notes behavior changes such as no margin check before a conditional order triggers.

This means a vendor bot claim, paper backtest, or screenshot cannot prove live readiness unless it supplies enough detail to replay against current exchange filters, order endpoints, and trigger behavior.

## Sources Checked

- Pionex DCA/Martingale Simple Mode help: https://support.pionex.com/hc/en-us/articles/49723823672089-DCA-Martingale-Bot-Simple-Mode
- Pionex Martingale guide: https://www.pionex.com/blog/whats-martingale-bot/
- Dynamic Grid Trading Strategy, arXiv 2506.11921: https://arxiv.org/html/2506.11921v1
- 3Commas DCA backtesting docs: https://help.3commas.io/en/articles/4829733-dca-bot-introduction-to-backtesting
- 3Commas strategy gallery docs: https://help.3commas.io/en/articles/14828053-strategy-gallery-how-to-find-and-launch-pre-built-dca-bot-strategies
- Bitsgap crypto backtesting guide: https://bitsgap.com/blog/crypto-backtesting-guide-2025-tools-tips-and-how-bitsgap-helps
- Bitsgap backtest help: https://bitsgap.com/helpdesk/article/10023850035612-Backtest-bot-efficiency-analysis
- Bitsgap 2026 grid strategy guide: https://bitsgap.com/blog/grid-trading-strategy-explained-how-to-profit-in-any-market-in-2026
- Phemex Q1 2026 bot strategy report: https://phemex.com/blogs/top-10-profitable-bot-strategies-q1-2026
- OKX Futures DCA/Martingale help: https://www.okx.com/help/whats-futures-dca-bot-and-how-do-i-maximize-my-efficiency-with-automated
- 3Commas DCA bots: https://3commas.io/dca-bots
- Neutralis DGT/market-making research summary: https://neutralis.finance/insights/market-making-beat-market-research
- Binance USD-M Futures exchange information: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information
- Binance USD-M Futures common symbol filters: https://developers.binance.com/docs/derivatives/usds-margined-futures/common-definition
- Binance USD-M Futures new algo order: https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order
- Binance derivatives change log: https://developers.binance.com/docs/derivatives/change-log

## Conclusion

External search did not uncover a hidden qualifying martingale/grid portfolio. The closest public high-return examples either fail drawdown by a wide margin or lack the evidence required to verify the original objective. Current public evidence therefore does not overturn the internal verdict: no C/B/A martingale-grid candidate is currently proven under the requested gates.
