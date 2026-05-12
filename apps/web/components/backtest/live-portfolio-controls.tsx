"use client";

import Link from "next/link";
import { startTransition, useEffect, useState } from "react";

import { requestBacktestApi } from "@/components/backtest/request-client";
import { MartingaleRiskWarning } from "@/components/backtest/martingale-risk-warning";
import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { Button } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable, type DataTableRow } from "@/components/ui/table";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type PortfolioRiskLimits = {
  max_global_budget_quote?: number | string | null;
  max_global_drawdown_quote?: number | string | null;
  max_symbol_budget_quote?: number | string | null;
  max_strategy_budget_quote?: number | string | null;
};

type PortfolioStrategy = {
  strategy_id: string;
  symbol: string;
  market: string;
  direction: string;
  runtime_status?: StrategyStatus | null;
  margin_mode?: string | null;
  leverage?: number | null;
  sizing?: Record<string, unknown> | null;
  spacing?: Record<string, unknown> | null;
  take_profit?: Record<string, unknown> | null;
  risk_limits?: PortfolioRiskLimits | null;
};

type PortfolioConfig = {
  direction_mode?: string;
  strategies: PortfolioStrategy[];
  risk_limits?: PortfolioRiskLimits | null;
};

type LivePortfolio = {
  portfolio_id: string;
  owner?: string;
  status: string;
  candidate_id?: string;
  risk_summary?: Record<string, unknown> | null;
  config: PortfolioConfig;
};

type StrategyStatus = "pending_confirmation" | "running" | "paused" | "stopped";
type StrategyStatusSource =
  | { kind: "explicit"; status: StrategyStatus }
  | { kind: "inherited"; status: StrategyStatus | null }
  | { kind: "local"; status: StrategyStatus };

export function MartingalePortfolioList({
  lang,
  locale,
}: {
  lang: UiLanguage;
  locale: string;
}) {
  const [portfolios, setPortfolios] = useState<LivePortfolio[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setError("");
      try {
        const response = await requestBacktestApi("/api/user/martingale-portfolios", {
          cache: "no-store",
        });

        if (cancelled) {
          return;
        }

        if (!response.ok) {
          setError(response.message);
          setPortfolios([]);
          return;
        }

        const next = Array.isArray(response.data) ? response.data.map(normalizePortfolio) : [];
        setPortfolios(next);
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <AppShellSection
      actions={
        <Link
          className="inline-flex items-center justify-center rounded-sm border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary"
          href={`/${locale}/app/backtest`}
        >
          {pickText(lang, "去回测台", "Open backtest desk")}
        </Link>
      }
      description={pickText(
        lang,
        "列表页直接汇总状态、预算上限、全局回撤阈值、活跃策略数，以及需要人工处理或孤儿实例风险。",
        "This list keeps status, equity cap, portfolio drawdown threshold, active strategy count, and operator/orphan warnings visible.",
      )}
      eyebrow={pickText(lang, "Martingale Portfolio", "Martingale Portfolio")}
      title={pickText(lang, "Portfolio 运行总览", "Portfolio operations")}
    >
      <MartingaleRiskWarning lang={lang} compact />
      {error ? (
        <StatusBanner
          description={error}
          lang={lang}
          title={pickText(lang, "加载 Portfolio 失败", "Failed to load portfolios")}
          tone="error"
        />
      ) : null}
      <div className="grid gap-4 xl:grid-cols-2">
        {loading ? (
          <LoadingCard lang={lang} />
        ) : portfolios.length === 0 ? (
          <Card className="xl:col-span-2">
            <CardHeader>
              <CardTitle>{pickText(lang, "还没有 Portfolio", "No portfolios yet")}</CardTitle>
              <CardDescription>
                {pickText(
                  lang,
                  "先在回测台发布候选，再回到这里管理运行状态。",
                  "Publish a candidate from the backtest desk first, then manage it here.",
                )}
              </CardDescription>
            </CardHeader>
          </Card>
        ) : (
          portfolios.map((portfolio) => {
            const warnings = warningMessages(lang, portfolio);
            return (
              <Card key={portfolio.portfolio_id}>
                <CardHeader className="gap-3">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="space-y-1">
                      <CardTitle>{portfolioName(portfolio)}</CardTitle>
                      <CardDescription>
                        {portfolio.config.strategies.map((strategy) => strategy.symbol).join(" / ") || portfolio.portfolio_id}
                      </CardDescription>
                    </div>
                    <Chip tone={statusTone(portfolio.status)}>{humanizeStatus(lang, portfolio.status)}</Chip>
                  </div>
                </CardHeader>
                <CardBody className="space-y-4">
                  <dl className="grid grid-cols-2 gap-3 text-sm">
                    <MetricBlock
                      label={pickText(lang, "总权益上限", "Total equity cap")}
                      value={formatQuote(readNumber(portfolio.config.risk_limits?.max_global_budget_quote), lang)}
                    />
                    <MetricBlock
                      label={pickText(lang, "全局回撤阈值", "Portfolio drawdown")}
                      value={formatQuote(readNumber(portfolio.config.risk_limits?.max_global_drawdown_quote), lang)}
                    />
                    <MetricBlock
                      label={pickText(lang, "明确运行策略", "Explicit running strategies")}
                      value={String(activeStrategyCount(portfolio))}
                    />
                    <MetricBlock
                      label={pickText(lang, "风险摘要", "Risk summary")}
                      value={riskSummaryText(lang, portfolio)}
                    />
                  </dl>
                  <div className="flex flex-wrap gap-2">
                    {warnings.map((warning) => (
                      <Chip key={warning.text} tone={warning.tone}>
                        {warning.text}
                      </Chip>
                    ))}
                  </div>
                  <div className="flex justify-end">
                    <Link
                      className="inline-flex items-center justify-center rounded-sm border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary"
                      href={`/${locale}/app/martingale-portfolios/${portfolio.portfolio_id}`}
                    >
                      {pickText(lang, "查看详情", "View detail")}
                    </Link>
                  </div>
                </CardBody>
              </Card>
            );
          })
        )}
      </div>
    </AppShellSection>
  );
}

export function MartingalePortfolioDetail({
  lang,
  locale,
  portfolioId,
}: {
  lang: UiLanguage;
  locale: string;
  portfolioId: string;
}) {
  const [portfolio, setPortfolio] = useState<LivePortfolio | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [strategyStatuses, setStrategyStatuses] = useState<Record<string, StrategyStatus>>({});

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setError("");
      try {
        const response = await requestBacktestApi(`/api/user/martingale-portfolios/${portfolioId}`, {
          cache: "no-store",
        });

        if (cancelled) {
          return;
        }

        if (!response.ok) {
          setError(response.message);
          setPortfolio(null);
          return;
        }

        setPortfolio(normalizePortfolio(response.data));
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();

    return () => {
      cancelled = true;
    };
  }, [portfolioId]);

  const groups = portfolio ? groupStrategies(portfolio, lang) : [];
  const warningBanners = portfolio ? warningMessages(lang, portfolio) : [];
  const exposureRows = portfolio ? buildExposureRows(lang, portfolio) : [];

  return (
    <AppShellSection
      actions={
        <Link
          className="inline-flex items-center justify-center rounded-sm border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary"
          href={`/${locale}/app/martingale-portfolios`}
        >
          {pickText(lang, "返回 Portfolio 列表", "Back to portfolios")}
        </Link>
      }
      description={pickText(
        lang,
        "按 symbol / direction 归组查看实例，盯住 symbol 暴露、全局回撤线，并在页内直接执行 Portfolio 或单策略操作。",
        "Inspect instances grouped by symbol and direction, monitor symbol exposure and portfolio drawdown, and execute portfolio or strategy operations in-page.",
      )}
      eyebrow={pickText(lang, "Live Portfolio", "Live Portfolio")}
      title={portfolio ? portfolioName(portfolio) : portfolioId}
    >
      <MartingaleRiskWarning lang={lang} compact />
      {loading ? <LoadingCard lang={lang} /> : null}
      {!loading && error ? (
        <StatusBanner
          description={error}
          lang={lang}
          title={pickText(lang, "加载 Portfolio 详情失败", "Failed to load portfolio detail")}
          tone="error"
        />
      ) : null}
      {!loading && portfolio ? (
        <>
          {warningBanners.map((warning) => (
            <StatusBanner
              description={warning.description}
              key={warning.text}
              lang={lang}
              title={warning.text}
              tone={warning.tone === "danger" ? "error" : warning.tone === "warning" ? "warning" : "info"}
            />
          ))}
          <div className="grid gap-4 lg:grid-cols-4">
            <StatCard
              label={pickText(lang, "Portfolio 状态", "Portfolio status")}
              value={humanizeStatus(lang, portfolio.status)}
            />
            <StatCard
              label={pickText(lang, "明确运行策略", "Explicit running strategies")}
              value={String(activeStrategyCount(portfolio))}
            />
            <StatCard
              label={pickText(lang, "Symbol 暴露", "Symbol exposure")}
              value={String(uniqueSymbols(portfolio).length)}
            />
            <StatCard
              label={pickText(lang, "全局回撤线", "Global portfolio drawdown")}
              value={formatQuote(readNumber(portfolio.config.risk_limits?.max_global_drawdown_quote), lang)}
            />
          </div>

          <Card>
            <CardHeader className="gap-3">
              <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                <div className="space-y-1">
                  <CardTitle>{pickText(lang, "Portfolio 操作", "Portfolio controls")}</CardTitle>
                  <CardDescription>
                    {pickText(
                      lang,
                      "当前“暂停新开仓”会复用 Portfolio pause 接口，把组合切到 paused；后端补齐专用 runtime 动作前，这里按临时语义展示。",
                      "The current pause-new-entries action reuses the portfolio pause endpoint and moves the portfolio into paused. Until a dedicated runtime action exists, the UI labels this as a temporary semantic fallback.",
                    )}
                  </CardDescription>
                </div>
                <LivePortfolioControls
                  entity={{ kind: "portfolio", portfolioId: portfolio.portfolio_id, status: portfolio.status }}
                  lang={lang}
                  onPortfolioChange={(next) => {
                    setPortfolio((current) => (current ? { ...current, status: next } : current));
                  }}
                />
              </div>
            </CardHeader>
          </Card>

          <div className="grid gap-4 xl:grid-cols-[minmax(0,1.5fr)_minmax(320px,0.9fr)]">
            <div className="space-y-4">
              {groups.map((group) => (
                <Card key={group.key}>
                  <CardHeader>
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="space-y-1">
                        <CardTitle>{group.symbol}</CardTitle>
                        <CardDescription>{group.label}</CardDescription>
                      </div>
                      <Chip tone="info">{pickText(lang, "实例组", "Instance group")}</Chip>
                    </div>
                  </CardHeader>
                  <CardBody className="space-y-4">
                    {group.items.map((strategy) => {
                      const statusSource = resolveStrategyStatus(
                        strategy,
                        portfolio.status,
                        strategy.strategy_id ? strategyStatuses[strategy.strategy_id] : undefined,
                      );
                      return (
                        <div className="rounded-sm border border-border/70 p-4" key={strategy.strategy_id || `${strategy.symbol}-${strategy.direction}`}>
                          <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                            <div className="space-y-3">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="text-sm font-semibold text-foreground">
                                  {strategy.strategy_id || pickText(lang, "未绑定实例 ID", "Missing strategy instance id")}
                                </span>
                                <Chip tone={strategyStatusTone(statusSource)}>{renderStrategyStatusLabel(lang, statusSource)}</Chip>
                              </div>
                              <p aria-live="polite" className="text-xs text-muted-foreground" role="status">
                                {renderStrategyStatusDescription(lang, statusSource)}
                              </p>
                              <dl className="grid grid-cols-2 gap-3 text-sm">
                                <MetricBlock
                                  label={pickText(lang, "方向", "Direction")}
                                  value={humanizeDirection(lang, strategy.direction)}
                                />
                                <MetricBlock
                                  label={pickText(lang, "市场", "Market")}
                                  value={humanizeMarket(lang, strategy.market)}
                                />
                                <MetricBlock
                                  label={pickText(lang, "首单", "First order")}
                                  value={formatQuote(readNumber(readSizingValue(strategy.sizing, "first_order_quote")), lang)}
                                />
                                <MetricBlock
                                  label={pickText(lang, "最大腿数", "Max legs")}
                                  value={String(readNumber(readSizingValue(strategy.sizing, "max_legs")) ?? "-")}
                                />
                                <MetricBlock
                                  label={pickText(lang, "杠杆 / 保证金", "Leverage / margin")}
                                  value={[
                                    strategy.leverage ? `${strategy.leverage}x` : null,
                                    strategy.margin_mode ? humanizeMarginMode(lang, strategy.margin_mode) : null,
                                  ].filter(Boolean).join(" · ") || "-"}
                                />
                                <MetricBlock
                                  label={pickText(lang, "单策略预算", "Strategy budget")}
                                  value={formatQuote(strategyBudget(strategy), lang)}
                                />
                              </dl>
                            </div>
                            <LivePortfolioControls
                              entity={{ kind: "strategy", strategyId: strategy.strategy_id, statusSource }}
                              lang={lang}
                              onStrategyChange={(next) => {
                                if (!strategy.strategy_id) {
                                  return;
                                }
                                setStrategyStatuses((current) => ({
                                  ...current,
                                  [strategy.strategy_id]: next,
                                }));
                              }}
                            />
                          </div>
                        </div>
                      );
                    })}
                  </CardBody>
                </Card>
              ))}
            </div>

            <div className="space-y-4">
              <Card>
                <CardHeader>
                  <CardTitle>{pickText(lang, "Symbol 暴露", "Symbol exposure")}</CardTitle>
                  <CardDescription>
                    {pickText(
                      lang,
                      "按 symbol 汇总预算与方向覆盖，方便复核集中暴露。",
                      "Budget and direction coverage are aggregated by symbol for quick concentration checks.",
                    )}
                  </CardDescription>
                </CardHeader>
                <CardBody>
                  <DataTable
                    columns={[
                      { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                      { key: "directions", label: pickText(lang, "方向", "Directions") },
                      { key: "budget", label: pickText(lang, "预算", "Budget"), align: "right" },
                    ]}
                    emptyMessage={pickText(lang, "暂无 symbol 暴露。", "No symbol exposure yet.")}
                    rows={exposureRows}
                  />
                </CardBody>
              </Card>

              <Card>
                <CardHeader>
                  <CardTitle>{pickText(lang, "Portfolio 风险边界", "Portfolio risk guardrails")}</CardTitle>
                </CardHeader>
                <CardBody className="space-y-2 text-sm text-muted-foreground">
                  <p>
                    {pickText(lang, "总权益上限", "Total equity cap")}:
                    {" "}
                    <span className="font-medium text-foreground">
                      {formatQuote(readNumber(portfolio.config.risk_limits?.max_global_budget_quote), lang)}
                    </span>
                  </p>
                  <p>
                    {pickText(lang, "全局回撤阈值", "Global drawdown threshold")}:
                    {" "}
                    <span className="font-medium text-foreground">
                      {formatQuote(readNumber(portfolio.config.risk_limits?.max_global_drawdown_quote), lang)}
                    </span>
                  </p>
                  <p>
                    {pickText(lang, "风险摘要", "Risk summary")}:
                    {" "}
                    <span className="font-medium text-foreground">{riskSummaryText(lang, portfolio)}</span>
                  </p>
                </CardBody>
              </Card>
            </div>
          </div>
        </>
      ) : null}
    </AppShellSection>
  );
}

function LivePortfolioControls({
  entity,
  lang,
  onPortfolioChange,
  onStrategyChange,
}: {
  entity:
    | { kind: "portfolio"; portfolioId: string; status: string }
    | { kind: "strategy"; strategyId: string; statusSource: StrategyStatusSource };
  lang: UiLanguage;
  onPortfolioChange?: (status: string) => void;
  onStrategyChange?: (status: StrategyStatus) => void;
}) {
  const [pending, setPending] = useState("");
  const [message, setMessage] = useState("");

  async function run(action: "pause" | "stop" | "start" | "resume") {
    setPending(action);
    setMessage("");

    let input = "";
    let init: RequestInit | undefined;

    if (entity.kind === "portfolio") {
      if (action === "pause") {
        input = `/api/user/martingale-portfolios/${entity.portfolioId}/pause`;
      } else if (action === "stop") {
        input = `/api/user/martingale-portfolios/${entity.portfolioId}/stop`;
      } else {
        input = `/api/user/martingale-portfolios/${entity.portfolioId}/confirm-start`;
      }
      init = { method: "POST" };
    } else if (action === "pause") {
      input = "/api/user/martingale-portfolios/strategies/pause";
      init = {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ ids: [entity.strategyId] }),
      };
    } else if (action === "resume") {
      input = `/api/user/martingale-portfolios/strategies/${entity.strategyId}/resume`;
      init = { method: "POST" };
    } else {
      input = `/api/user/martingale-portfolios/strategies/${entity.strategyId}/stop`;
      init = { method: "POST" };
    }

    try {
      const response = await requestBacktestApi(input, init);

      if (!response.ok) {
        setMessage(response.message);
        return;
      }

      if (entity.kind === "portfolio") {
        const nextStatus = action === "pause" ? "paused" : action === "stop" ? "stopped" : "running";
        startTransition(() => onPortfolioChange?.(nextStatus));
        return;
      }

      const nextStatus = action === "pause" ? "paused" : action === "stop" ? "stopped" : "running";
      setMessage(
        pickText(
          lang,
          `本地临时状态：${humanizeStatus(lang, nextStatus)}，等待后端同步。`,
          `Local temporary status: ${humanizeStatus(lang, nextStatus)}. Awaiting backend sync.`,
        ),
      );
      startTransition(() => onStrategyChange?.(nextStatus));
    } finally {
      setPending("");
    }
  }

  const inferredStatus = entity.kind === "strategy" ? effectiveStrategyControlStatus(entity.statusSource) : null;

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-2">
        {entity.kind === "portfolio" && (entity.status === "pending_confirmation" || entity.status === "paused") ? (
          <Button
            aria-busy={pending === "start"}
            disabled={pending !== ""}
            onClick={() => void run("start")}
            size="sm"
            type="button"
          >
            {entity.status === "pending_confirmation"
              ? pickText(lang, "启动 Portfolio", "Start portfolio")
              : pickText(lang, "恢复运行", "Resume running")}
          </Button>
        ) : null}
        {entity.kind === "portfolio" && entity.status === "running" ? (
          <Button
            aria-busy={pending === "pause"}
            disabled={pending !== ""}
            onClick={() => void run("pause")}
            size="sm"
            tone="outline"
            type="button"
          >
            {pickText(lang, "暂停新开仓", "Pause new entries")}
          </Button>
        ) : null}
        {entity.kind === "strategy" && inferredStatus === "running" ? (
          <Button
            aria-busy={pending === "pause"}
            disabled={pending !== "" || !entity.strategyId}
            onClick={() => void run("pause")}
            size="sm"
            tone="outline"
            type="button"
          >
            {pickText(lang, "暂停策略", "Pause strategy")}
          </Button>
        ) : null}
        {entity.kind === "strategy" && inferredStatus === "paused" ? (
          <Button
            aria-busy={pending === "resume"}
            disabled={pending !== "" || !entity.strategyId}
            onClick={() => void run("resume")}
            size="sm"
            type="button"
          >
            {pickText(lang, "恢复策略", "Resume strategy")}
          </Button>
        ) : null}
        {((entity.kind === "portfolio" && entity.status !== "stopped") ||
          (entity.kind === "strategy" && inferredStatus !== "stopped")) ? (
          <Button
            aria-busy={pending === "stop"}
            disabled={pending !== "" || (entity.kind === "strategy" && !entity.strategyId)}
            onClick={() => void run("stop")}
            size="sm"
            tone="danger"
            type="button"
          >
            {entity.kind === "portfolio"
              ? pickText(lang, "停止 Portfolio", "Stop portfolio")
              : pickText(lang, "停止策略", "Stop strategy")}
          </Button>
        ) : null}
      </div>
      {message ? <p aria-live="polite" className="text-xs text-destructive" role="status">{message}</p> : null}
    </div>
  );
}

function normalizePortfolio(value: unknown): LivePortfolio {
  const source = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
  const configSource = source.config && typeof source.config === "object" ? (source.config as Record<string, unknown>) : {};
  const strategiesSource = Array.isArray(configSource.strategies) ? configSource.strategies : [];

  return {
    portfolio_id: String(source.portfolio_id ?? "unknown-portfolio"),
    owner: typeof source.owner === "string" ? source.owner : undefined,
    status: typeof source.status === "string" ? source.status : "pending_confirmation",
    candidate_id: typeof source.candidate_id === "string" ? source.candidate_id : undefined,
    risk_summary: source.risk_summary && typeof source.risk_summary === "object"
      ? (source.risk_summary as Record<string, unknown>)
      : null,
    config: {
      direction_mode: typeof configSource.direction_mode === "string" ? configSource.direction_mode : undefined,
      risk_limits: normalizeRiskLimits(configSource.risk_limits),
      strategies: strategiesSource.map((entry, index) => normalizeStrategy(entry, index)),
    },
  };
}

function normalizeStrategy(value: unknown, index: number): PortfolioStrategy {
  const source = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
  return {
    strategy_id: typeof source.strategy_id === "string" ? source.strategy_id : "",
    symbol: typeof source.symbol === "string" ? source.symbol : "UNKNOWN",
    market: typeof source.market === "string" ? source.market : "spot",
    direction: typeof source.direction === "string" ? source.direction : "long",
    runtime_status: readStrategyStatus(source.runtime_status) ?? readStrategyStatus(source.status),
    margin_mode: typeof source.margin_mode === "string" ? source.margin_mode : null,
    leverage: readNumber(source.leverage),
    sizing: source.sizing && typeof source.sizing === "object" ? (source.sizing as Record<string, unknown>) : null,
    spacing: source.spacing && typeof source.spacing === "object" ? (source.spacing as Record<string, unknown>) : null,
    take_profit: source.take_profit && typeof source.take_profit === "object"
      ? (source.take_profit as Record<string, unknown>)
      : null,
    risk_limits: normalizeRiskLimits(source.risk_limits),
  };
}

function normalizeRiskLimits(value: unknown): PortfolioRiskLimits | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  return value as PortfolioRiskLimits;
}

function portfolioName(portfolio: LivePortfolio) {
  const leadSymbol = portfolio.config.strategies[0]?.symbol ?? portfolio.portfolio_id;
  return `Martingale Portfolio ${leadSymbol}`;
}

function activeStrategyCount(portfolio: LivePortfolio) {
  return portfolio.config.strategies.filter((strategy) => strategy.runtime_status === "running").length;
}

function warningMessages(lang: UiLanguage, portfolio: LivePortfolio) {
  const warnings: Array<{ text: string; description: string; tone: "info" | "warning" | "danger" }> = [];
  if (portfolio.status === "paused" || portfolio.status === "stopped") {
    warnings.push({
      text: pickText(lang, "需要处理", "Needs attention"),
      description: pickText(
        lang,
        "组合当前不是 running，建议复核是否需要恢复或直接停止相关策略实例。",
        "The portfolio is not running. Review whether it should resume or whether the related strategy instances should stop.",
      ),
      tone: portfolio.status === "stopped" ? "danger" : "warning",
    });
  }

  const orphanCount = portfolio.config.strategies.filter((strategy) => strategy.strategy_id.trim() === "").length;
  if (orphanCount > 0) {
    warnings.push({
      text: pickText(lang, "孤儿实例告警", "Orphan warning"),
      description: pickText(
        lang,
        `发现 ${orphanCount} 个 strategy_id 缺失，单实例操作前要先确认后端绑定。`,
        `Found ${orphanCount} strategies without strategy_id. Confirm backend binding before running instance operations.`,
      ),
      tone: "danger",
    });
  }

  if (warnings.length === 0) {
    warnings.push({
      text: pickText(lang, "运行平稳", "No orphan warnings"),
      description: pickText(lang, "当前未发现额外人工处理信号。", "No extra operator warnings were detected."),
      tone: "info",
    });
  }

  return warnings;
}

function riskSummaryText(lang: UiLanguage, portfolio: LivePortfolio) {
  const symbols = uniqueSymbols(portfolio).length;
  const maxLeverage = readNumber(portfolio.risk_summary?.max_leverage) ?? maxStrategyLeverage(portfolio);
  return [
    `${symbols} ${pickText(lang, "个 symbol", "symbols")}`,
    maxLeverage ? `${pickText(lang, "最高", "max")} ${maxLeverage}x` : null,
    portfolio.risk_summary?.requires_futures ? pickText(lang, "含合约", "futures") : pickText(lang, "纯现货/混合", "spot or mixed"),
  ].filter(Boolean).join(" · ");
}

function uniqueSymbols(portfolio: LivePortfolio) {
  return Array.from(new Set(portfolio.config.strategies.map((strategy) => strategy.symbol)));
}

function maxStrategyLeverage(portfolio: LivePortfolio) {
  return portfolio.config.strategies.reduce<number | null>((highest, strategy) => {
    if (!strategy.leverage) {
      return highest;
    }
    if (highest === null || strategy.leverage > highest) {
      return strategy.leverage;
    }
    return highest;
  }, null);
}

function groupStrategies(portfolio: LivePortfolio, lang: UiLanguage) {
  const groups = new Map<string, { key: string; label: string; symbol: string; items: PortfolioStrategy[] }>();

  for (const strategy of portfolio.config.strategies) {
    const key = `${strategy.symbol}:${strategy.direction}`;
    const current = groups.get(key);
    if (current) {
      current.items.push(strategy);
      continue;
    }
    groups.set(key, {
      key,
      symbol: strategy.symbol,
      label: `${humanizeDirection(lang, strategy.direction)} / ${humanizeMarket(lang, strategy.market)}`,
      items: [strategy],
    });
  }

  return Array.from(groups.values());
}

function buildExposureRows(lang: UiLanguage, portfolio: LivePortfolio): DataTableRow[] {
  const summary = new Map<string, { directions: Set<string>; budget: number | null }>();

  for (const strategy of portfolio.config.strategies) {
    const current = summary.get(strategy.symbol) ?? { directions: new Set<string>(), budget: 0 };
    current.directions.add(humanizeDirection(lang, strategy.direction));
    const budget = strategyBudget(strategy);
    current.budget = (current.budget ?? 0) + (budget ?? 0);
    summary.set(strategy.symbol, current);
  }

  return Array.from(summary.entries()).map(([symbol, entry]) => ({
    id: symbol,
    symbol,
    directions: Array.from(entry.directions).join(" / "),
    budget: formatQuote(entry.budget, lang),
  }));
}

function strategyBudget(strategy: PortfolioStrategy) {
  return (
    readNumber(strategy.risk_limits?.max_strategy_budget_quote)
    ?? readNumber(strategy.risk_limits?.max_symbol_budget_quote)
    ?? readNumber(readSizingValue(strategy.sizing, "max_budget_quote"))
    ?? readNumber(readSizingValue(strategy.sizing, "first_order_quote"))
    ?? null
  );
}

function readSizingValue(
  sizing: Record<string, unknown> | null | undefined,
  key: "first_order_quote" | "max_budget_quote" | "max_legs",
) {
  if (!sizing) {
    return null;
  }
  return sizing[key];
}

function readNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function formatQuote(value: number | null, lang: UiLanguage) {
  if (value === null) {
    return "-";
  }
  return new Intl.NumberFormat(lang === "zh" ? "zh-CN" : "en-US", {
    maximumFractionDigits: 2,
    minimumFractionDigits: value < 10 ? 2 : 0,
  }).format(value);
}

function statusTone(status: string) {
  if (status === "running") {
    return "success" as const;
  }
  if (status === "paused" || status === "pending_confirmation") {
    return "warning" as const;
  }
  if (status === "stopped") {
    return "danger" as const;
  }
  return "default" as const;
}

function humanizeStatus(lang: UiLanguage, status: string) {
  switch (status) {
    case "running":
      return pickText(lang, "运行中", "Running");
    case "paused":
      return pickText(lang, "已暂停", "Paused");
    case "stopped":
      return pickText(lang, "已停止", "Stopped");
    case "pending_confirmation":
      return pickText(lang, "待确认启动", "Pending confirmation");
    default:
      return status;
  }
}

function readStrategyStatus(value: unknown): StrategyStatus | null {
  if (
    value === "pending_confirmation" ||
    value === "running" ||
    value === "paused" ||
    value === "stopped"
  ) {
    return value;
  }
  return null;
}

function inferPortfolioStrategyStatus(portfolioStatus: string): StrategyStatus | null {
  if (portfolioStatus === "stopped") {
    return "stopped";
  }
  if (portfolioStatus === "paused") {
    return "paused";
  }
  if (portfolioStatus === "pending_confirmation") {
    return "pending_confirmation";
  }
  return "running";
}

function resolveStrategyStatus(
  strategy: PortfolioStrategy,
  portfolioStatus: string,
  localOverride?: StrategyStatus,
): StrategyStatusSource {
  if (localOverride) {
    return { kind: "local", status: localOverride };
  }
  if (strategy.runtime_status) {
    return { kind: "explicit", status: strategy.runtime_status };
  }
  return { kind: "inherited", status: inferPortfolioStrategyStatus(portfolioStatus) };
}

function effectiveStrategyControlStatus(statusSource: StrategyStatusSource) {
  return statusSource.status;
}

function strategyStatusTone(statusSource: StrategyStatusSource) {
  if (statusSource.kind === "inherited" && statusSource.status === null) {
    return "default" as const;
  }
  return statusTone(statusSource.status ?? "unknown");
}

function renderStrategyStatusLabel(lang: UiLanguage, statusSource: StrategyStatusSource) {
  if (statusSource.kind === "local") {
    return pickText(
      lang,
      `本地临时状态：${humanizeStatus(lang, statusSource.status)}`,
      `Local temporary status: ${humanizeStatus(lang, statusSource.status)}`,
    );
  }
  if (statusSource.kind === "explicit") {
    return humanizeStatus(lang, statusSource.status);
  }
  if (statusSource.status === null) {
    return pickText(lang, "等待后端同步", "Awaiting backend sync");
  }
  return pickText(
    lang,
    `继承组合状态：${humanizeStatus(lang, statusSource.status)}`,
    `Inherited portfolio status: ${humanizeStatus(lang, statusSource.status)}`,
  );
}

function renderStrategyStatusDescription(lang: UiLanguage, statusSource: StrategyStatusSource) {
  if (statusSource.kind === "local") {
    return pickText(
      lang,
      "这是前端刚提交操作后的本地临时状态，等待后端同步。",
      "This is a local temporary status after the latest action, awaiting backend sync.",
    );
  }
  if (statusSource.kind === "explicit") {
    return pickText(
      lang,
      "该状态来自后端返回的单策略 runtime 状态。",
      "This value comes from the backend strategy runtime status.",
    );
  }
  return pickText(
    lang,
    "后端尚未返回单策略 runtime 状态，当前先继承组合状态展示。",
    "The backend has not returned a per-strategy runtime status yet, so the UI is using an inherited portfolio fallback.",
  );
}

function humanizeDirection(lang: UiLanguage, direction: string) {
  return direction === "short" ? pickText(lang, "做空", "Short") : pickText(lang, "做多", "Long");
}

function humanizeMarket(lang: UiLanguage, market: string) {
  return market === "usd_m_futures" ? pickText(lang, "U 本位合约", "USD-M Futures") : pickText(lang, "现货", "Spot");
}

function humanizeMarginMode(lang: UiLanguage, marginMode: string) {
  return marginMode === "cross" ? pickText(lang, "全仓", "Cross") : pickText(lang, "逐仓", "Isolated");
}

function MetricBlock({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-sm border border-border/60 bg-background/40 p-3">
      <dt className="text-xs font-medium uppercase tracking-wider text-muted-foreground">{label}</dt>
      <dd className="mt-1 text-sm font-semibold text-foreground">{value}</dd>
    </div>
  );
}

function StatCard({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <Card>
      <CardHeader className="space-y-2">
        <CardDescription>{label}</CardDescription>
        <CardTitle className="text-2xl">{value}</CardTitle>
      </CardHeader>
    </Card>
  );
}

function LoadingCard({ lang }: { lang: UiLanguage }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{pickText(lang, "Portfolio 加载中", "Loading portfolios")}</CardTitle>
        <CardDescription>
          {pickText(lang, "正在向运行时和发布服务同步状态。", "Syncing state from runtime and publish services.")}
        </CardDescription>
      </CardHeader>
    </Card>
  );
}
