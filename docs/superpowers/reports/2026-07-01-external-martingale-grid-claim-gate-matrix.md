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

## 2026-07-01 Supplemental Web Sweep

A supplemental sweep checked additional 2026 vendor and education pages after the live-promotion gate was added. It also found no replayable candidate:

- 3Commas' current public homepage shows eye-catching sample strategy cards such as `116.6%` ROI with `-2.10%` max floating drawdown, `133.1%` ROI with `-4.51%` max floating drawdown, and `92.4%` ROI with `-1.51%` max floating drawdown. These are not documented as martingale/grid portfolios and do not include trade-level evidence, capital, symbols, exact windows, 2023-2026 segments, or Binance live-parity details.
- 3Commas' DCA pages document multi-pair DCA, backtesting, strategy cards, APY, minimum deposit, and max floating drawdown filters. This proves tooling exists, not that a public card satisfies the requested C/B/A gates across all required periods.
- Phemex's same Q1 2026 report includes BTC futures grid typical metrics of `45-95%` estimated APR with `5-9%` max drawdown. This is grid, not martingale; it is a single quarter, lacks 2023-2026 segment proof, and does not reach the aggressive `>110%` target.
- BingX Martingale/DCA documentation confirms USDT-M futures support and independent strategy capital accounting, but provides product parameters rather than a verified candidate with the required return/DD/segment evidence.
- Altrady, Coinbase, HaasOnline, Bitsgap, and ChainUp grid explainers consistently describe the same range-break risk: grid bots work in ranges and can stop trading or hold losses when price breaks the configured range. These are risk notes, not qualifying portfolios.
- Stoic's 2026 DCA risk article frames DCA as execution logic that assumes volatility/mean reversion and can fail when the asset never recovers, the market enters a prolonged downtrend, or capital is exhausted before recovery. Bitsgap's May 2026 setup-mistake article similarly warns that DCA without stop-loss/review can keep accumulating exposure and exhaust capital. These support the capital/DD gate concern rather than providing a pass.

## Gate Matrix

| Source | Public claim | Gate result | Why it does not qualify |
|---|---:|---|---|
| Pionex DCA/Martingale Simple Mode help page | BTC/USDT backtest from 2020-04-01 to 2021-07-01; default AI parameters report `205.68% APR` and `-52.84%` max drawdown | Reject | Annualized return is high, but DD fails all C/B/A gates, it is single-symbol BTC/USDT, wrong period, no 2023H1/H2/2024/2025/2026_ytd segment proof, no `<5000U` capital proof, and not Binance USD-M live-parity evidence. |
| Pionex conservative DCA/Martingale example | More conservative 10% drop scale reports `122.12%` profit and `-16.37%` max drawdown | Reject | DD fails conservative `<=10%`; the source does not clearly state this as annualized return, and it still lacks required multi-symbol, period-segment, capital, and Binance-live evidence. |
| Dynamic Grid Trading arXiv 2506.11921 | BTC/ETH 1m backtest from 2021-01 to 2024-07; DGT reaches roughly `60-70%` IRR in favorable parameter regions | Reject | This can be relevant to conservative return, but not balanced/aggressive targets; the paper notes ETH DGT drawdown around `50%` when the market declined around `80%`, which fails the DD gates. It also lacks the requested 2025/2026_ytd segments and live-execution parity. |
| 3Commas homepage sample strategy cards | Public cards show high ROI and low max floating drawdown examples, including `116.6% / -2.10%`, `133.1% / -4.51%`, and `92.4% / -1.51%` | Reject | These are marketing/sample strategy cards, not a disclosed martingale/grid portfolio. They lack exact parameters, capital, trade list, multi-symbol allocation, required 2023-2026 segments, and Binance-live reproducibility. |
| 3Commas DCA/backtest documentation | Platform supports DCA bot backtesting and exposes APY, minimum deposit, max floating drawdown, and performance charts | Not evidence | This describes tooling, not a public candidate satisfying the objective. No complete parameter set, trade list, segment proof, or current Binance USD-M live replay is provided. |
| 3Commas Strategy Gallery docs | Strategy cards expose APY, ROI/drawdown filters, minimum deposit, max floating drawdown, and backtest charts | Not evidence | Useful discovery tooling, but the public documentation does not supply a replayable candidate that satisfies all C/B/A gates and segment/live constraints. |
| Bitsgap grid/DCA/backtest documentation | Platform supports grid/DCA backtests; Bitsgap warns that deep drawdowns such as `30-40%` are high-risk for live trading | Not evidence | This is useful risk context, but not a qualifying martingale/grid portfolio. It does not disclose a C/B/A candidate with the required periods and capital/live constraints. |
| Phemex bot strategy report | States Futures DCA/Martingale bots carry the highest drawdown risk among Phemex bot types | Reject as candidate | Risk note supports the internal diagnosis; it does not provide a qualifying low-DD, high-return, multi-period candidate. |
| Phemex BTC futures grid Q1 2026 metrics | Typical Q1 2026 BTC futures grid metrics list `45-95%` estimated APR and `5-9%` max drawdown | Reject | Quarter-only grid evidence, not a martingale portfolio; no 2023-2026 segment proof, no multi-symbol allocation, no trade list, no Binance-live parity, and upper return still misses the aggressive target. |
| BingX Futures Martingale/DCA docs | USDT-M futures Martingale/DCA product parameters and independent strategy capital accounting | Not evidence | Product documentation only. It does not disclose return/DD/segment proof for a qualifying candidate. |
| Altrady/Coinbase/HaasOnline grid risk explainers | Grid bots are described as range-dependent and vulnerable when price exits the configured range | Not evidence | These sources reinforce the same risk model but do not provide a qualifying strategy. |
| Stoic/Bitsgap DCA risk explainers | DCA reacts to adverse price movement by accumulating exposure, and can fail if recovery does not happen before capital or risk limits are exhausted | Not evidence | Risk explanation only; no candidate, no multi-period proof, and no live-parity evidence. |

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
- 3Commas homepage strategy cards: https://3commas.io/
- 3Commas strategy gallery docs: https://help.3commas.io/en/articles/14828053-strategy-gallery-how-to-find-and-launch-pre-built-dca-bot-strategies
- BingX Martingale/DCA parameters: https://bingx.com/en/support/articles/11359359478799
- BingX 2026 bot overview: https://bingx.com/en/learn/article/what-are-the-best-crypto-trading-bots
- Altrady grid bot: https://www.altrady.com/features/grid-bot
- Altrady best grid bots 2026: https://www.altrady.com/blog/crypto-bots/best-grid-trading-bots
- Stoic DCA bot risk explanation: https://stoic.ai/blog/how-dca-bots-work-in-crypto-markets-mechanics-risks-and-smarter-alternatives/
- Bitsgap bot setup mistakes: https://bitsgap.com/blog/why-your-crypto-trading-bot-is-not-making-profit-7-setup-mistakes-to-check
- Coinbase grid bot risks: https://www.coinbase.com/learn/advanced-trading/what-is-a-grid-trading-bot-and-how-does-it-work
- HaasOnline grid bot risks: https://haasonline.com/grid-trading-bot
- ChainUp grid bot risk controls: https://www.chainup.com/blog/what-is-grid-trading-bot/
- Neutralis DGT/market-making research summary: https://neutralis.finance/insights/market-making-beat-market-research
- Binance USD-M Futures exchange information: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information
- Binance USD-M Futures common symbol filters: https://developers.binance.com/docs/derivatives/usds-margined-futures/common-definition
- Binance USD-M Futures new algo order: https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order
- Binance derivatives change log: https://developers.binance.com/docs/derivatives/change-log

## Conclusion

External search did not uncover a hidden qualifying martingale/grid portfolio. The closest public high-return examples either fail drawdown by a wide margin or lack the evidence required to verify the original objective. Current public evidence therefore does not overturn the internal verdict: no C/B/A martingale-grid candidate is currently proven under the requested gates.
