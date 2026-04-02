import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, FormStack } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { firstValue } from "../../../lib/auth";

type TelegramPageProps = {
  searchParams?: Promise<{
    bound?: string | string[];
    code?: string | string[];
    generate?: string | string[];
  }>;
};

const deliveryRows = [
  { id: "notice-1", event: "Membership expiring", channel: "Telegram + web", state: "Queued" },
  { id: "notice-2", event: "Runtime failure", channel: "Telegram + web", state: "Delivered" },
  { id: "notice-3", event: "Deposit confirmed", channel: "Telegram + web", state: "Delivered" },
];

export default async function TelegramPage({ searchParams }: TelegramPageProps) {
  const params = (await searchParams) ?? {};
  const generated = firstValue(params.generate) === "1";
  const code = firstValue(params.code) ?? (generated ? "GB-4821" : "");
  const bound = firstValue(params.bound) === "1";

  return (
    <>
      <StatusBanner
        description="Telegram notifications cover strategy lifecycle, API incidents, membership reminders, and deposit confirmations."
        title="Telegram bind flow"
        tone="warning"
      />
      {bound ? (
        <StatusBanner
          description="Telegram bound. Critical alerts now reach the linked account and the web inbox together."
          title="Telegram bound"
          tone="success"
        />
      ) : null}
      <AppShellSection
        description="Generate a one-time bind code in the web app, then send it to the Telegram bot to complete binding."
        eyebrow="Telegram bind"
        title="Telegram Notifications"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Bind your Telegram account</CardTitle>
              <CardDescription>One user binds one Telegram identity only.</CardDescription>
            </CardHeader>
            <CardBody>
              {!generated ? (
                <FormStack action="/app/telegram" method="get">
                  <Button name="generate" type="submit" value="1">
                    Generate bind code
                  </Button>
                </FormStack>
              ) : (
                <>
                  <p>Bind code</p>
                  <p>
                    <strong>{code}</strong>
                  </p>
                  <p>Send <strong>/start {code}</strong> to the Telegram bot.</p>
                  <FormStack action="/app/telegram" method="get">
                    <input name="generate" type="hidden" value="1" />
                    <input name="code" type="hidden" value={code} />
                    <Button name="bound" type="submit" value="1">
                      I sent the code to the bot
                    </Button>
                  </FormStack>
                </>
              )}
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Notification coverage</CardTitle>
              <CardDescription>Critical alerts are mirrored in web and Telegram.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                <li>Strategy started and paused</li>
                <li>API credential issue</li>
                <li>Membership reminder</li>
                <li>Deposit success and per-fill PnL</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>Recent delivery events</CardTitle>
          <CardDescription>Delivery history stays visible even after the bind is complete.</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "event", label: "Event" },
              { key: "channel", label: "Channel" },
              { key: "state", label: "State", align: "right" },
            ]}
            rows={deliveryRows.map((row) => ({
              ...row,
              state: <Chip tone={row.state === "Delivered" ? "success" : "warning"}>{row.state}</Chip>,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}
