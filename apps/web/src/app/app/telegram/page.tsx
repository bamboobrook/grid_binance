import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, FormStack } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type TelegramPageProps = {
  searchParams?: Promise<{
    code?: string | string[];
    error?: string | string[];
    expires?: string | string[];
    notice?: string | string[];
  }>;
};

type TelegramBindingStatus = {
  email: string;
  bound: boolean;
  bound_at: string | null;
  chat_id: string | null;
  telegram_user_id: string | null;
};

type NotificationInboxResponse = {
  email: string;
  items: Array<{
    event: { kind: string; title: string };
    telegram_delivered: boolean;
    in_app_delivered: boolean;
  }>;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function TelegramPage({ searchParams }: TelegramPageProps) {
  const params = (await searchParams) ?? {};
  const notice = firstValue(params.notice);
  const error = firstValue(params.error);
  const bindCode = firstValue(params.code) ?? "";
  const expiresAt = firstValue(params.expires) ?? "";
  const context = await fetchTelegramContext();
  const binding = context?.binding ?? null;
  const inbox = context?.notifications.items ?? [];

  return (
    <>
      <StatusBanner
        description="Telegram notifications cover strategy lifecycle, API incidents, membership reminders, and deposit confirmations."
        title="Telegram bind flow"
        tone="warning"
      />
      {error ? <StatusBanner description={error} title="Telegram action failed" tone="warning" /> : null}
      {notice === "bind-code-issued" ? (
        <StatusBanner
          description="Send the issued code to the Telegram bot to finish linking, then refresh this page after the bot replies."
          title="Bind code issued"
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
              {binding?.bound ? (
                <ul className="text-list">
                  <li>Telegram bound at: {binding.bound_at ?? "-"}</li>
                  <li>Telegram chat id: {binding.chat_id ?? "-"}</li>
                  <li>Telegram user id: {binding.telegram_user_id ?? "-"}</li>
                  <li>Web inbox and Telegram are both active.</li>
                </ul>
              ) : bindCode ? (
                <>
                  <p>A fresh bind code is ready.</p>
                  <p>
                    <strong>{bindCode}</strong>
                  </p>
                  <p>Send <strong>/start {bindCode}</strong> to the Telegram bot.</p>
                  <p>Expires at: {expiresAt || "shortly"}</p>
                  <FormStack action="/api/user/telegram" method="post">
                    <Button name="intent" type="submit" value="generate">
                      Generate new bind code
                    </Button>
                  </FormStack>
                </>
              ) : (
                <FormStack action="/api/user/telegram" method="post">
                  <Button name="intent" type="submit" value="generate">
                    Generate bind code
                  </Button>
                </FormStack>
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
            rows={inbox.map((row, index) => {
              const delivery = describeDelivery(row, Boolean(binding?.bound));
              return {
                id: `${row.event.kind}-${index}`,
                event: row.event.title,
                channel: delivery.channel,
                state: <Chip tone={delivery.tone}>{delivery.state}</Chip>,
              };
            })}
          />
        </CardBody>
      </Card>
    </>
  );
}

async function fetchTelegramContext() {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }

  const profile = await fetchProfile(sessionToken);
  if (!profile) {
    return null;
  }

  const [binding, notifications] = await Promise.all([
    fetchBindingStatus(sessionToken, profile.email),
    fetchNotifications(sessionToken, profile.email),
  ]);

  return { binding, notifications };
}

async function fetchProfile(sessionToken: string): Promise<{ email: string } | null> {
  const response = await fetch(`${authApiBaseUrl()}/profile`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  const payload = (await response.json()) as { email?: string };
  return typeof payload.email === "string" ? { email: payload.email } : null;
}

async function fetchBindingStatus(sessionToken: string, email: string) {
  const response = await fetch(`${authApiBaseUrl()}/telegram/binding?email=${encodeURIComponent(email)}`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as TelegramBindingStatus;
}

async function fetchNotifications(sessionToken: string, email: string) {
  const response = await fetch(`${authApiBaseUrl()}/notifications?email=${encodeURIComponent(email)}`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return { email, items: [] } as NotificationInboxResponse;
  }
  return (await response.json()) as NotificationInboxResponse;
}

function describeDelivery(
  row: NotificationInboxResponse["items"][number],
  bound: boolean,
) {
  if (row.telegram_delivered) {
    return { channel: "Telegram + web", state: "Delivered", tone: "success" as const };
  }
  if (bound) {
    return { channel: "Web only", state: "Failed", tone: "warning" as const };
  }
  return { channel: "Web only", state: "Not bound", tone: "info" as const };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
