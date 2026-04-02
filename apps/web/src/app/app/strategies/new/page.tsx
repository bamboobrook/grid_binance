import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { DialogFrame } from "../../../../components/ui/dialog";
import { Button, Field, FormStack, Input, Select } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { getCurrentUserProductState } from "../../../../lib/api/user-product-state";

export default async function StrategyNewPage() {
  const state = await getCurrentUserProductState();

  return (
    <>
      <StatusBanner
        description="Draft creation now captures the main lifecycle inputs before pre-flight and start."
        title="Strategy creation workspace"
        tone="info"
      />
      {state.flash.strategy === "Draft saved" ? (
        <StatusBanner description="A user-owned draft was created and copied into your strategy library." title="Draft saved" tone="success" />
      ) : null}
      <AppShellSection
        description="Create a draft first, then save edits, run pre-flight, and start from the strategy workspace."
        eyebrow="Strategy creation"
        title="New Strategy"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Draft setup</CardTitle>
              <CardDescription>New drafts generate their own route id and remain user-owned after creation.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/strategies/create" method="post">
                <Field label="Strategy name">
                  <Input defaultValue="ETH Swing Builder" name="name" required />
                </Field>
                <Field label="Symbol">
                  <Input defaultValue="ETHUSDT" name="symbol" required />
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
                    <option value="long">Long futures grid</option>
                    <option value="short">Short futures grid</option>
                    <option value="neutral">Neutral futures grid</option>
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
                <Field label="Post-trigger behavior">
                  <Select defaultValue="rebuild" name="postTrigger">
                    <option value="stop">Stop after execution</option>
                    <option value="rebuild">Rebuild and continue</option>
                  </Select>
                </Field>
                <Button type="submit">Save draft</Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Composer rules</CardTitle>
              <CardDescription>Drafts align with the same lifecycle rules enforced on detail pages.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Futures allow only one strategy per user per symbol per direction.</li>
                <li>Running strategy parameters cannot be hot-modified.</li>
                <li>Trailing take profit uses taker execution and may increase fees.</li>
                <li>Existing user drafts: {state.strategies.length}</li>
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
