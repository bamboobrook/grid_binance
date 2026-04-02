import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, FormStack } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

const deliveryRows = [
  { id: "notice-1", event: "Membership expiring", channel: "Telegram + web", state: "Queued" },
  { id: "notice-2", event: "Runtime failure", channel: "Telegram + web", state: "Delivered" },
  { id: "notice-3", event: "Deposit confirmed", channel: "Telegram + web", state: "Delivered" },
];

export default async function TelegramPage() {
  const state = await getCurrentUserProductState();

  return (
    <>
      <StatusBanner
        description="Telegram notifications cover strategy lifecycle, API incidents, membership reminders, and deposit confirmations."
        title="Telegram bind flow"
        tone="warning"
      />
      {state.flash.telegram ? (
        <StatusBanner
          description={state.telegram.state === "bound" ? "Critical alerts now reach the linked account and the web inbox together." : "Send the issued code to the Telegram bot to finish linking."}
          title={state.flash.telegram}
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
              {state.telegram.state === "unbound" ? (
                <FormStack action="/api/user/telegram" method="post">
                  <Button name="intent" type="submit" value="generate">
                    Generate bind code
                  </Button>
                </FormStack>
              ) : state.telegram.state === "code_issued" ? (
                <>
                  <p>A fresh bind code is ready.</p>
                  <p>
                    <strong>{state.telegram.bindCode}</strong>
                  </p>
                  <p>Send <strong>/start {state.telegram.bindCode}</strong> to the Telegram bot.</p>
                  <p>Issued at: {state.telegram.bindCodeIssuedAt}</p>
                  <FormStack action="/api/user/telegram" method="post">
                    <Button name="intent" type="submit" value="confirm">
                      I sent the code to the bot
                    </Button>
                  </FormStack>
                </>
              ) : (
                <ul className="text-list">
                  <li>Telegram bound at: {state.telegram.boundAt}</li>
                  <li>Latest bind code: {state.telegram.bindCode}</li>
                  <li>Web inbox and Telegram are both active.</li>
                </ul>
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
