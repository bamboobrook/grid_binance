import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { DialogFrame } from "../../../../components/ui/dialog";
import { Button, Field, FormStack, Input, Select } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";

export default function StrategyNewPage() {
  return (
    <>
      <StatusBanner
        description="Draft creation now includes symbol, market, mode, and risk-preparation fields instead of a placeholder shell."
        title="Strategy creation workspace"
        tone="info"
      />
      <AppShellSection
        description="Create a draft first, then save edits, run pre-flight, and start from the strategy workspace."
        eyebrow="Strategy creation"
        title="New Strategy"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Draft setup</CardTitle>
              <CardDescription>Captured values are forwarded into the strategy workspace for edit and pre-flight review.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/app/strategies/grid-btc" method="get">
                <input name="draft" type="hidden" value="1" />
                <Field label="Strategy name">
                  <Input defaultValue="BTC Recovery Ladder" name="name" required />
                </Field>
                <Field label="Symbol">
                  <Input defaultValue="BTCUSDT" name="symbol" required />
                </Field>
                <Field label="Market type">
                  <Select defaultValue="spot" name="marketType">
                    <option value="spot">Spot</option>
                    <option value="usd-m">USDⓈ-M futures</option>
                    <option value="coin-m">COIN-M futures</option>
                  </Select>
                </Field>
                <Field label="Strategy mode">
                  <Select defaultValue="classic" name="mode">
                    <option value="classic">Classic two-way spot</option>
                    <option value="buy-only">Buy-only spot grid</option>
                    <option value="sell-only">Sell-only spot grid</option>
                  </Select>
                </Field>
                <Field label="Generation mode">
                  <Select defaultValue="geometric" name="generation">
                    <option value="arithmetic">Arithmetic</option>
                    <option value="geometric">Geometric</option>
                    <option value="custom">Fully custom</option>
                  </Select>
                </Field>
                <Field label="Trailing take profit (%)">
                  <Input defaultValue="0.8" inputMode="decimal" name="trailing" />
                </Field>
                <Button type="submit">Save draft</Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Composer rules</CardTitle>
              <CardDescription>Draft inputs are validated later by pre-flight, but the warnings stay visible here.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Futures allow only one strategy per user per symbol per direction.</li>
                <li>Running strategy parameters cannot be hot-modified.</li>
                <li>Trailing take profit uses taker execution and may increase fees.</li>
                <li>Templates are copied into user-owned drafts and can be edited freely afterward.</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <DialogFrame
        description="Starting is blocked until pre-flight confirms exchange filters, balance, and the required hedge-mode posture."
        title="Pre-flight remains mandatory"
        tone="warning"
      />
    </>
  );
}
