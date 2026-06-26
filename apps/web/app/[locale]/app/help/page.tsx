import Link from "next/link";
import { cookies } from "next/headers";
import { ChevronRight } from "lucide-react";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute, type UiLanguage } from "@/lib/ui/preferences";

type HelpPageProps = {
  params: Promise<{ locale: string }>;
};

type HelpAction = {
  href: string;
  label: string;
};

type HelpQuestion = {
  actions?: HelpAction[];
  answer: string;
  bullets?: string[];
  question: string;
};

type HelpGroup = {
  description: string;
  questions: HelpQuestion[];
  title: string;
};

type HelpShortcut = {
  description: string;
  href: string;
  label: string;
  title: string;
};

export default async function HelpPage({ params }: HelpPageProps) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const shortcuts = helpShortcuts(lang, locale);
  const steps = setupSteps(lang, locale);
  const groups = helpGroups(lang, locale);

  return (
    <AppShellSection
      eyebrow={pickText(lang, "帮助", "Help")}
      title={pickText(lang, "常见问题", "FAQ")}
    >
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
        {shortcuts.map((item) => (
          <Link
            className="rounded-sm border border-border bg-card p-4 text-card-foreground shadow-sm transition-colors hover:bg-secondary"
            href={item.href}
            key={item.href}
          >
            <span className="block text-sm font-semibold">{item.title}</span>
            <span className="mt-2 block text-sm leading-6 text-muted-foreground">{item.description}</span>
            <span className="mt-3 inline-flex text-xs font-bold text-primary">{item.label}</span>
          </Link>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "开机器人流程", "Bot setup flow")}</CardTitle>
          <CardDescription>{pickText(lang, "按这个顺序走，第一次会更稳。", "Follow this order for a calmer first run.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-4">
            {steps.map((step, index) => (
              <Link
                className="rounded-sm border border-border bg-background/40 p-3 transition-colors hover:bg-secondary"
                href={step.href}
                key={step.title}
              >
                <span className="text-xs font-bold text-primary">{String(index + 1).padStart(2, "0")}</span>
                <span className="mt-2 block text-sm font-semibold text-foreground">{step.title}</span>
                <span className="mt-1 block text-xs leading-5 text-muted-foreground">{step.description}</span>
              </Link>
            ))}
          </div>
        </CardBody>
      </Card>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        {groups.map((group) => (
          <Card key={group.title}>
            <CardHeader>
              <CardTitle>{group.title}</CardTitle>
              <CardDescription>{group.description}</CardDescription>
            </CardHeader>
            <CardBody className="space-y-3">
              {group.questions.map((item) => (
                <HelpDisclosure item={item} key={item.question} />
              ))}
            </CardBody>
          </Card>
        ))}
      </div>
    </AppShellSection>
  );
}

function HelpDisclosure({ item }: { item: HelpQuestion }) {
  return (
    <details className="group rounded-sm border border-border bg-background/40 open:bg-secondary/30">
      <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-4 py-3 text-sm font-semibold text-foreground">
        <span>{item.question}</span>
        <ChevronRight className="h-4 w-4 shrink-0 text-muted-foreground transition-transform group-open:rotate-90" />
      </summary>
      <div className="border-t border-border px-4 py-3">
        <p className="text-sm leading-6 text-muted-foreground">{item.answer}</p>
        {item.bullets?.length ? (
          <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
            {item.bullets.map((bullet) => (
              <li key={bullet}>- {bullet}</li>
            ))}
          </ul>
        ) : null}
        {item.actions?.length ? (
          <div className="mt-3 flex flex-wrap gap-2">
            {item.actions.map((action) => (
              <Link
                className="inline-flex h-8 items-center justify-center rounded-sm border border-border bg-card px-3 text-xs font-semibold text-foreground transition-colors hover:bg-secondary"
                href={action.href}
                key={action.href + action.label}
              >
                {action.label}
              </Link>
            ))}
          </div>
        ) : null}
      </div>
    </details>
  );
}

function appPath(locale: string, path: string) {
  return `/${locale}${path}`;
}

function helpShortcuts(lang: UiLanguage, locale: string): HelpShortcut[] {
  return [
    {
      title: pickText(lang, "第一次使用", "First run"),
      description: pickText(lang, "先连币安，再小额开一个普通网格。", "Connect Binance, then start one small ordinary grid."),
      href: appPath(locale, "/app/exchange"),
      label: pickText(lang, "开始连接", "Connect"),
    },
    {
      title: pickText(lang, "机器人状态", "Bot status"),
      description: pickText(lang, "看运行、暂停、异常和盈亏。", "Check running, paused, blocked, and PnL states."),
      href: appPath(locale, "/app/strategies"),
      label: pickText(lang, "查看机器人", "View bots"),
    },
    {
      title: pickText(lang, "订单成交", "Orders"),
      description: pickText(lang, "核对挂单、成交和交易所记录。", "Review open orders, fills, and exchange records."),
      href: appPath(locale, "/app/orders"),
      label: pickText(lang, "查看记录", "View records"),
    },
    {
      title: pickText(lang, "提醒和会员", "Alerts"),
      description: pickText(lang, "处理到期、异常和 Telegram 提醒。", "Handle expiry, incidents, and Telegram alerts."),
      href: appPath(locale, "/app/notifications"),
      label: pickText(lang, "查看提醒", "View alerts"),
    },
  ];
}

function setupSteps(lang: UiLanguage, locale: string): HelpShortcut[] {
  return [
    {
      title: pickText(lang, "连接币安", "Connect Binance"),
      description: pickText(lang, "只开读取和交易权限，不开提现。", "Use read and trade permissions only. No withdrawals."),
      href: appPath(locale, "/app/exchange"),
      label: "",
    },
    {
      title: pickText(lang, "测试模板", "Backtest"),
      description: pickText(lang, "先看回撤、成交次数和资金占用。", "Check drawdown, fill count, and capital use first."),
      href: appPath(locale, "/app/backtest"),
      label: "",
    },
    {
      title: pickText(lang, "创建机器人", "Create bot"),
      description: pickText(lang, "新手先用小资金普通网格。", "Start with a small ordinary grid."),
      href: appPath(locale, "/app/strategies/new"),
      label: "",
    },
    {
      title: pickText(lang, "观察结果", "Monitor"),
      description: pickText(lang, "先看订单成交，再看收益统计。", "Review orders first, then PnL stats."),
      href: appPath(locale, "/app/orders"),
      label: "",
    },
  ];
}

function helpGroups(lang: UiLanguage, locale: string): HelpGroup[] {
  return [
    {
      title: pickText(lang, "开始前", "Before You Start"),
      description: pickText(lang, "连接交易所、API 权限和第一笔小额试跑。", "Exchange connection, API scope, and first small run."),
      questions: [
        {
          question: pickText(lang, "第一次开机器人应该按什么顺序？", "What is the right order for the first bot?"),
          answer: pickText(
            lang,
            "先连接币安，确认连接正常；再用模板创建小额普通网格；最后观察订单和收益。",
            "Connect Binance, confirm the connection, create a small ordinary grid from a template, then watch orders and PnL.",
          ),
          bullets: [
            pickText(lang, "不要一上来扩大资金。", "Do not start with a large budget."),
            pickText(lang, "先跑通流程，再调整参数。", "Prove the flow before tuning settings."),
          ],
          actions: [
            { href: appPath(locale, "/app/exchange"), label: pickText(lang, "连接币安", "Connect Binance") },
            { href: appPath(locale, "/app/strategies/new"), label: pickText(lang, "创建机器人", "Create bot") },
          ],
        },
        {
          question: pickText(lang, "API 权限怎么设置才安全？", "Which API permissions are safe?"),
          answer: pickText(
            lang,
            "只需要读取和交易权限。不要开启提现权限；保存后先运行连接测试。",
            "Use read and trade permissions only. Do not enable withdrawals. Run the connection test after saving.",
          ),
          actions: [
            { href: appPath(locale, "/app/exchange"), label: pickText(lang, "检查 API", "Check API") },
            { href: appPath(locale, "/app/security"), label: pickText(lang, "账户安全", "Security") },
          ],
        },
        {
          question: pickText(lang, "为什么建议先小额试跑？", "Why start with a small test?"),
          answer: pickText(
            lang,
            "小额试跑能先确认余额、挂单、成交和提醒是否正常。确认没问题后，再扩大资金。",
            "A small test confirms balance, open orders, fills, and alerts. Increase capital only after everything looks normal.",
          ),
          actions: [
            { href: appPath(locale, "/app/orders"), label: pickText(lang, "看订单成交", "Orders & fills") },
            { href: appPath(locale, "/app/notifications"), label: pickText(lang, "看提醒", "Alerts") },
          ],
        },
      ],
    },
    {
      title: pickText(lang, "创建机器人", "Create Bots"),
      description: pickText(lang, "网格类型、价格区间、资金和回测。", "Grid style, price range, budget, and backtests."),
      questions: [
        {
          question: pickText(lang, "普通网格、合约网格、马丁怎么选？", "Ordinary grid, futures grid, or DCA?"),
          answer: pickText(
            lang,
            "普通网格适合震荡行情；合约网格适合有方向判断并能承受杠杆风险；马丁更适合分批补仓思路。",
            "Ordinary grid fits ranging markets. Futures grid needs directional judgment and leverage risk tolerance. DCA is for staged averaging.",
          ),
          actions: [
            { href: appPath(locale, "/app/strategies/new"), label: pickText(lang, "选择模板", "Choose template") },
            { href: appPath(locale, "/app/strategies/new?strategyType=martingale_grid"), label: pickText(lang, "马丁策略", "DCA strategy") },
          ],
        },
        {
          question: pickText(lang, "价格区间和网格数量怎么定？", "How do I choose range and grid count?"),
          answer: pickText(
            lang,
            "区间覆盖你认为会来回波动的价格带。网格越多，成交更频繁，但单格利润更薄。",
            "The range should cover the price band you expect to oscillate in. More grids trade more often but reduce profit per grid.",
          ),
          bullets: [
            pickText(lang, "新手优先使用模板默认值。", "Beginners should start with template defaults."),
            pickText(lang, "回测时同时看收益和最大回撤。", "Review both return and max drawdown in backtests."),
          ],
          actions: [
            { href: appPath(locale, "/app/backtest"), label: pickText(lang, "先做回测", "Backtest first") },
          ],
        },
        {
          question: pickText(lang, "回测结果先看哪几个数字？", "Which backtest numbers matter first?"),
          answer: pickText(
            lang,
            "先看最大回撤、交易次数、资金占用，再看总收益。只看最高收益容易忽略风险。",
            "Check max drawdown, fill count, and capital use before total return. Looking only at the highest return hides risk.",
          ),
          actions: [
            { href: appPath(locale, "/app/backtest"), label: pickText(lang, "查看回测", "Open backtest") },
            { href: appPath(locale, "/app/analytics"), label: pickText(lang, "收益统计", "PnL stats") },
          ],
        },
      ],
    },
    {
      title: pickText(lang, "运行中", "While Running"),
      description: pickText(lang, "状态、订单、成交、异常阻塞。", "Status, orders, fills, and blocked bots."),
      questions: [
        {
          question: pickText(lang, "机器人是否正常看哪里？", "Where do I check if a bot is healthy?"),
          answer: pickText(
            lang,
            "先看我的机器人里的状态和盈亏，再看订单成交里的挂单和成交历史。",
            "Check status and PnL in My Bots first, then open Orders for open orders and fill history.",
          ),
          actions: [
            { href: appPath(locale, "/app/strategies"), label: pickText(lang, "我的机器人", "My Bots") },
            { href: appPath(locale, "/app/orders"), label: pickText(lang, "订单成交", "Orders") },
          ],
        },
        {
          question: pickText(lang, "成交了但收益没马上增加，是异常吗？", "A fill happened but PnL did not rise. Is that wrong?"),
          answer: pickText(
            lang,
            "不一定。网格收益通常要结合成对买卖、均价和费用一起看。先确认成交记录，再看收益统计。",
            "Not always. Grid PnL depends on paired buys and sells, average price, and fees. Confirm fills first, then review PnL stats.",
          ),
          actions: [
            { href: appPath(locale, "/app/orders"), label: pickText(lang, "成交历史", "Fill history") },
            { href: appPath(locale, "/app/analytics"), label: pickText(lang, "收益统计", "PnL stats") },
          ],
        },
        {
          question: pickText(lang, "异常阻塞时应该怎么处理？", "What should I do when a bot is blocked?"),
          answer: pickText(
            lang,
            "先看提醒和机器人状态，不要急着扩大资金。常见原因是 API、余额、会员或交易所限制。",
            "Check alerts and bot status first. Do not increase capital. Common causes are API, balance, membership, or exchange limits.",
          ),
          actions: [
            { href: appPath(locale, "/app/notifications"), label: pickText(lang, "查看提醒", "View alerts") },
            { href: appPath(locale, "/app/strategies"), label: pickText(lang, "处理机器人", "Manage bot") },
          ],
        },
      ],
    },
    {
      title: pickText(lang, "资金和风险", "Funds & Risk"),
      description: pickText(lang, "投入金额、止盈止损、会员到期。", "Budget, take profit, stop loss, and membership expiry."),
      questions: [
        {
          question: pickText(lang, "需要准备多少资金？", "How much capital do I need?"),
          answer: pickText(
            lang,
            "至少要覆盖多个网格的买入资金，并留出余额应对波动。新手先从小额开始。",
            "Keep enough balance for several grid buys and leave spare funds for volatility. Beginners should start small.",
          ),
          actions: [
            { href: appPath(locale, "/app/strategies/new"), label: pickText(lang, "设置资金", "Set budget") },
          ],
        },
        {
          question: pickText(lang, "止盈止损怎么用更稳？", "How do I use take profit and stop loss safely?"),
          answer: pickText(
            lang,
            "止盈用于收住目标收益，止损用于限制继续亏损。第一次先用模板或保守参数。",
            "Take profit locks a target gain. Stop loss limits further loss. Use template or conservative settings at first.",
          ),
          actions: [
            { href: appPath(locale, "/app/backtest"), label: pickText(lang, "测试参数", "Test settings") },
          ],
        },
        {
          question: pickText(lang, "会员到期后会影响机器人吗？", "Does membership expiry affect bots?"),
          answer: pickText(
            lang,
            "到期后会进入宽限期，已运行机器人可继续一段时间，但新的启动会受限制。建议提前续费。",
            "After expiry, a grace period starts. Running bots may continue briefly, but new starts are limited. Renew early.",
          ),
          actions: [
            { href: appPath(locale, "/app/billing"), label: pickText(lang, "会员中心", "Membership") },
            { href: appPath(locale, "/app/notifications"), label: pickText(lang, "到期提醒", "Expiry alerts") },
          ],
        },
      ],
    },
    {
      title: pickText(lang, "账户和提醒", "Account & Alerts"),
      description: pickText(lang, "通知、两步验证、导出成交记录。", "Alerts, two-step verification, and fill export."),
      questions: [
        {
          question: pickText(lang, "怎么收到重要提醒？", "How do I receive important alerts?"),
          answer: pickText(
            lang,
            "站内提醒默认开启。需要手机及时收到，就在提醒页绑定 Telegram。",
            "In-app alerts are on by default. Bind Telegram on the Alerts page for timely mobile delivery.",
          ),
          actions: [
            { href: appPath(locale, "/app/notifications"), label: pickText(lang, "提醒设置", "Alert settings") },
          ],
        },
        {
          question: pickText(lang, "如何保护账号？", "How do I protect my account?"),
          answer: pickText(
            lang,
            "使用独立强密码，开启两步验证。币安 API 不要开启提现权限。",
            "Use a unique strong password and enable two-step verification. Never enable withdrawal permission on your Binance API key.",
          ),
          actions: [
            { href: appPath(locale, "/app/security"), label: pickText(lang, "账户安全", "Account security") },
            { href: appPath(locale, "/app/exchange"), label: pickText(lang, "检查 API", "Check API") },
          ],
        },
        {
          question: pickText(lang, "成交记录可以导出吗？", "Can I export fill records?"),
          answer: pickText(
            lang,
            "可以。在订单成交页导出成交 CSV，用于自己复盘或记账。",
            "Yes. Export fills as CSV from Orders for review or accounting.",
          ),
          actions: [
            { href: appPath(locale, "/app/orders"), label: pickText(lang, "导出成交", "Export fills") },
          ],
        },
      ],
    },
  ];
}
