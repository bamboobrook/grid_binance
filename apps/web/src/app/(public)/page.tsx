import Link from "next/link";
import { cookies } from "next/headers";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { StatusBanner } from "../../components/ui/status-banner";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "../../lib/ui/preferences";

export default async function HomePage() {
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);

  const pricingPlans = [
    {
      label: pickText(lang, "月付", "Monthly"),
      value: pickText(lang, "20 美元等值", "20 USD equivalent"),
      detail: pickText(lang, "单月权限，包含完整用户端、计费和 Telegram 通知能力。", "Single-month access with full user app, billing, and Telegram coverage."),
    },
    {
      label: pickText(lang, "季付", "Quarterly"),
      value: pickText(lang, "折合每月 18 美元", "18 USD equivalent per month"),
      detail: pickText(lang, "适合持续交易用户，续费成本更低。", "Three-month renewal stack for active traders who want lower monthly cost."),
    },
    {
      label: pickText(lang, "年付", "Yearly"),
      value: pickText(lang, "折合每月 15 美元", "15 USD equivalent per month"),
      detail: pickText(lang, "适合长期使用者，综合成本最低。", "Twelve-month term for long-horizon operators who want the lowest effective rate."),
    },
  ];

  const riskCopy = [
    pickText(lang, "不要给币安 API 开启提现权限。", "Do not enable withdrawal permission on your Binance API key."),
    pickText(lang, "充值金额必须完全一致，否则会进入人工审核。", "Payment amount must match exactly or the order moves into manual review."),
    pickText(lang, "追踪止盈会使用 taker 成交，手续费可能更高。", "Trailing take profit uses taker execution and may increase fees."),
    pickText(lang, "合约策略必须先满足双向持仓模式，预检才能通过。", "Futures strategies require hedge mode before pre-flight can pass."),
  ];

  const operatingRules = [
    pickText(lang, "每个用户只能绑定一个币安账号。", "One user can bind only one Binance account."),
    pickText(lang, "开通会员后才允许启动策略。", "Membership is required before any strategy can start."),
    pickText(lang, "已运行策略只能在 48 小时宽限期内继续。", "Existing running strategies may continue only through the 48-hour grace period."),
    pickText(lang, "运行异常会自动暂停策略，并推送网页与 Telegram 告警。", "Runtime failures auto-pause the affected strategy and push web plus Telegram alerts."),
  ];

  return (
    <>
      <StatusBanner
        description={pickText(lang, "在注册前，价格、交易所风险和充值规则都会明确展示。", "Public pricing, exchange risk, and billing warnings stay explicit before registration.")}
        title={pickText(lang, "运行前必须开通会员", "Membership required before runtime")}
        tone="warning"
      />
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>{pickText(lang, "面向币安用户的商用网格交易平台", "Commercial Grid Trading For Binance Users")}</CardTitle>
            <CardDescription>
              {pickText(lang, "支持现货、U 本位、币本位网格策略，并把计费边界、交易所检查和恢复流程直接展示给用户。", "Operate spot, USDⓈ-M, and COIN-M grid strategies with visible billing guardrails, exchange checks, and recovery workflows.")}
            </CardDescription>
          </CardHeader>
          <CardBody>
            <p>
              {pickText(lang, "链上充值提醒、会员宽限期规则、策略预检结果都会直观展示，不会被自动化流程掩盖。", "The product keeps chain payment warnings, membership grace-period rules, and pre-flight trading checks in front of the user instead of hiding them behind automation.")}
            </p>
            <div className="button-row">
              <Link className="button" href="/register">
                {pickText(lang, "创建账号", "Create account")}
              </Link>
              <Link className="button button--ghost" href="/login">
                {pickText(lang, "登录", "Login")}
              </Link>
            </div>
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "运行边界", "Operational guardrails")}</CardTitle>
            <CardDescription>{pickText(lang, "首版上线仍坚持把关键约束讲清楚。", "Commercial launch copy aligned with the March 31 design baseline.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {operatingRules.map((rule) => (
                <li key={rule}>{rule}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
      <section className="content-grid content-grid--metrics">
        {pricingPlans.map((plan) => (
          <Card key={plan.label}>
            <CardHeader>
              <CardTitle>{plan.label}</CardTitle>
              <CardDescription>{plan.value}</CardDescription>
            </CardHeader>
            <CardBody>{plan.detail}</CardBody>
          </Card>
        ))}
      </section>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "显式风险提示", "Visible risk copy")}</CardTitle>
            <CardDescription>{pickText(lang, "这些提醒会同时出现在落地页、计费页和策略工作台。", "Critical warnings stay on the landing page, billing page, and strategy workspace.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {riskCopy.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>{pickText(lang, "从正确入口开始", "Start with the right path")}</CardTitle>
            <CardDescription>{pickText(lang, "用户会从注册进入验证、交易所接入、计费、策略草稿和帮助中心。", "Users move from registration into exchange setup, billing, strategy draft, and help.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>
                <Link href="/register">{pickText(lang, "先完成邮箱注册与验证", "Register with email verification baseline")}</Link>
              </li>
              <li>
                <Link href="/login">{pickText(lang, "查看登录和安全提醒", "Review login and security prompts")}</Link>
              </li>
              <li>
                <Link href="/help/expiry-reminder">{pickText(lang, "阅读到期提醒说明", "Read the expiry reminder article")}</Link>
              </li>
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
