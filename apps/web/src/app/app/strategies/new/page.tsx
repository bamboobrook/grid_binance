import { AppShellSection } from "../../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { getStrategyComposerSnapshot } from "../../../../lib/api/server";

export default async function StrategyNewPage() {
  const snapshot = await getStrategyComposerSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="The documented /app/strategies/new route now exists inside the shared user shell with reusable form primitives."
        eyebrow="Strategy creation"
        title="New strategy"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Draft setup</CardTitle>
              <CardDescription>Task 7 stops at shell and form-system structure, not full validation logic.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="#" method="post">
                <Field label="Strategy name">
                  <Input name="name" placeholder="BTC mean re-entry" />
                </Field>
                <Field label="Market type">
                  <Select defaultValue="spot" name="marketType">
                    <option value="spot">Spot</option>
                    <option value="usd-m">USDⓈ-M futures</option>
                    <option value="coin-m">COIN-M futures</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button type="submit">Save draft</Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Supported modes</CardTitle>
              <CardDescription>Shell-aligned preview of the composer surface.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {snapshot.modes.map((item) => (
                  <li key={item.label}>
                    <strong>{item.label}:</strong> {item.value}
                  </li>
                ))}
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
