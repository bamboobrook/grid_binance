import Link from "next/link";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { StatusBanner } from "../../components/ui/status-banner";

const pricingPlans = [
  { label: "Monthly", value: "20 USD equivalent", detail: "Single-month access with full user app, billing, and Telegram coverage." },
  { label: "Quarterly", value: "18 USD equivalent per month", detail: "Three-month renewal stack for active traders who want lower monthly cost." },
  { label: "Yearly", value: "15 USD equivalent per month", detail: "Twelve-month term for long-horizon operators who want the lowest effective rate." },
];

const riskCopy = [
  "Do not enable withdrawal permission on your Binance API key.",
  "Payment amount must match exactly or the order moves into manual review.",
  "Trailing take profit uses taker execution and may increase fees.",
  "Futures strategies require hedge mode before pre-flight can pass.",
];

const operatingRules = [
  "One user can bind only one Binance account.",
  "Membership is required before any strategy can start.",
  "Existing running strategies may continue only through the 48-hour grace period.",
  "Runtime failures auto-pause the affected strategy and push web plus Telegram alerts.",
];

export default function HomePage() {
  return (
    <>
      <StatusBanner
        description="Public pricing, exchange risk, and billing warnings stay explicit before registration."
        title="Membership required before runtime"
        tone="warning"
      />
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>Commercial Grid Trading For Binance Users</CardTitle>
            <CardDescription>
              Operate spot, USDⓈ-M, and COIN-M grid strategies with visible billing guardrails, exchange checks, and
              recovery workflows.
            </CardDescription>
          </CardHeader>
          <CardBody>
            <p>
              The product keeps chain payment warnings, membership grace-period rules, and pre-flight trading checks in
              front of the user instead of hiding them behind automation.
            </p>
            <div className="button-row">
              <Link className="button" href="/register">
                Create account
              </Link>
              <Link className="button button--ghost" href="/login">
                Login
              </Link>
            </div>
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Operational guardrails</CardTitle>
            <CardDescription>Commercial launch copy aligned with the March 31 design baseline.</CardDescription>
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
            <CardTitle>Visible risk copy</CardTitle>
            <CardDescription>Critical warnings stay on the landing page, billing page, and strategy workspace.</CardDescription>
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
            <CardTitle>Start with the right path</CardTitle>
            <CardDescription>Users move from registration into exchange setup, billing, strategy draft, and help.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>
                <Link href="/register">Register with email verification baseline</Link>
              </li>
              <li>
                <Link href="/login">Review login and security prompts</Link>
              </li>
              <li>
                <Link href="/help/expiry-reminder">Read the expiry reminder article</Link>
              </li>
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
