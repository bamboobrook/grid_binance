import Link from "next/link";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { Tabs } from "../../../components/ui/tabs";
import { getPublicAuthSnapshot } from "../../../lib/api/server";
import { firstValue, safeRedirectTarget } from "../../../lib/auth";

type LoginPageProps = {
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
    notice?: string | string[];
    security?: string | string[];
    totp?: string | string[];
    adminBootstrap?: string | string[];
  }>;
};

const reminders = [
  "Membership expiry reminders appear in web and Telegram before grace ends.",
  "Do not enable withdrawal permission on Binance API keys.",
  "TOTP can be enabled later from the security center.",
];

function noticeCopy(notice: string | undefined) {
  switch (notice) {
    case "email-verified":
      return {
        title: "Email verified",
        description: "Your email is verified. Continue with password login and enter TOTP if prompted.",
      };
    case "password-reset-complete":
      return {
        title: "Password reset complete",
        description: "Use the new password to sign in.",
      };
    case "password-updated":
      return {
        title: "Password updated",
        description: "The previous session was revoked. Sign in again with the new password.",
      };
    case "totp-disabled":
      return {
        title: "TOTP disabled",
        description: "The previous session was revoked. Sign in again without a TOTP challenge.",
      };
    default:
      return null;
  }
}

export default async function LoginPage({ searchParams }: LoginPageProps) {
  const snapshot = await getPublicAuthSnapshot("login");
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const next = safeRedirectTarget(firstValue(params.next), "/app/dashboard");
  const notice = noticeCopy(firstValue(params.notice) ?? firstValue(params.security));
  const showTotp = firstValue(params.totp) === "1" || Boolean(error && /totp/i.test(error));
  const showAdminBootstrap = firstValue(params.adminBootstrap) === "1" || Boolean(error && /admin totp setup required/i.test(error ?? ""));

  return (
    <>
      <Tabs
        activeHref="/login"
        items={[
          { href: "/login", label: "Login" },
          { href: "/register", label: "Register" },
        ]}
        label="Authentication pages"
      />
      {error ? (
        <StatusBanner description={error} title="Login failed" tone="danger" />
      ) : notice ? (
        <StatusBanner description={notice.description} title={notice.title} tone="success" />
      ) : (
        <StatusBanner description={snapshot.notice.description} title={snapshot.notice.title} tone={snapshot.notice.tone} />
      )}
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>{snapshot.title}</CardTitle>
            <CardDescription>{snapshot.description}</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/login" method="post">
              <input name="next" type="hidden" value={next} />
              <Field hint="Use the verified email tied to your membership and exchange setup." label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field hint="Password login remains the first step before optional TOTP challenges." label="Password">
                <Input autoComplete="current-password" name="password" required type="password" />
              </Field>
              {showTotp ? (
                <Field hint="Enter the current 6-digit code from your authenticator app." label="TOTP code">
                  <Input autoComplete="one-time-code" inputMode="numeric" name="totpCode" pattern="[0-9]{6}" />
                </Field>
              ) : null}
              <div className="chip-row">
                {snapshot.checklist.map((item) => (
                  <Chip key={item} tone="info">
                    {item}
                  </Chip>
                ))}
              </div>
              <ButtonRow>
                <Button type="submit">{snapshot.submitLabel}</Button>
                <Link className="button button--ghost" href="/password-reset">
                  Reset password
                </Link>
                <Link className="button button--ghost" href="/help/expiry-reminder">
                  Review expiry reminders
                </Link>
                {showAdminBootstrap ? (
                  <Link className="button button--ghost" href={`/admin-bootstrap?email=${encodeURIComponent(email)}`}>
                    Bootstrap admin TOTP
                  </Link>
                ) : null}
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href={snapshot.alternateHref}>{snapshot.alternateLabel}</Link>
          </CardFooter>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Before you sign in</CardTitle>
            <CardDescription>Commercial guardrails stay visible on the public auth pages too.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {reminders.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
