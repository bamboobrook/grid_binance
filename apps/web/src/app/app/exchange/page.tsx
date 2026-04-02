import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DialogFrame } from "../../../components/ui/dialog";
import { Button, ButtonRow, Field, FormStack, Input, Select } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { Tabs } from "../../../components/ui/tabs";
import { getExchangeSnapshot } from "../../../lib/api/server";

export default async function ExchangePage() {
  const snapshot = await getExchangeSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        actions={<Tabs activeHref="/app/exchange" items={snapshot.tabs} />}
        description="Credential management, symbol metadata, and futures constraints share one consistent form and warning system."
        eyebrow="Exchange settings"
        title="Exchange"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Binance account binding</CardTitle>
              <CardDescription>Secrets stay masked after save and users can bind only one exchange account.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="#" method="post">
                <Field hint="Withdrawal permission must remain disabled." label="API key">
                  <Input defaultValue="AKIA••••••9XQ2" name="apiKey" />
                </Field>
                <Field hint="Stored encrypted server-side; never shown in plaintext." label="API secret">
                  <Input defaultValue="••••••••••••••••••" name="apiSecret" type="password" />
                </Field>
                <Field hint="Required for futures strategies in V1." label="Position mode">
                  <Select defaultValue="hedge" name="positionMode">
                    <option value="hedge">Hedge mode</option>
                    <option value="one-way">One-way</option>
                  </Select>
                </Field>
                <ButtonRow>
                  <Button type="submit">Save credentials</Button>
                  <Button tone="secondary" type="button">
                    Run connection test
                  </Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Metadata sync</CardTitle>
              <CardDescription>Symbol search and exchange scope are driven by scheduled sync.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {snapshot.metadata.map((item) => (
                  <li key={item.label}>
                    <strong>{item.label}:</strong> {item.value}
                  </li>
                ))}
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <DialogFrame
        description="If futures leverage or margin settings conflict with exchange state, pre-flight should fail fast and show the exact reason."
        title="Trading-critical warning"
        tone="warning"
      />
    </>
  );
}
