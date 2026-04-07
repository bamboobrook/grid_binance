import Link from "next/link";
import { cookies } from "next/headers";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { Tabs } from "../../../components/ui/tabs";
import { firstValue } from "../../../lib/auth";

type PageProps = {
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    setup?: string | string[];
  }>;
};

const PENDING_ADMIN_TOTP_SECRET_COOKIE = "pending_admin_totp_secret";
const PENDING_ADMIN_TOTP_CODE_COOKIE = "pending_admin_totp_code";
const PENDING_ADMIN_TOTP_EMAIL_COOKIE = "pending_admin_totp_email";

export default async function AdminBootstrapPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const setup = firstValue(params.setup) === "ready";
  const cookieStore = await cookies();
  const secret = cookieStore.get(PENDING_ADMIN_TOTP_SECRET_COOKIE)?.value ?? "";
  const code = cookieStore.get(PENDING_ADMIN_TOTP_CODE_COOKIE)?.value ?? "";
  const bootstrapEmail = cookieStore.get(PENDING_ADMIN_TOTP_EMAIL_COOKIE)?.value ?? email;

  return (
    <>
      <Tabs
        activeHref="/admin-bootstrap"
        items={[
          { href: "/login", label: "Login" },
          { href: "/register", label: "Register" },
          { href: "/admin-bootstrap", label: "Admin 2FA" },
        ]}
        label="Authentication pages"
      />
      {error ? (
        <StatusBanner description={error} title="Admin TOTP bootstrap failed" tone="danger" />
      ) : setup && secret ? (
        <StatusBanner description="Store the secret in your authenticator app, then use the shown code to complete the first admin login." title="Admin TOTP ready" tone="success" />
      ) : (
        <StatusBanner description="Configured admin accounts must complete TOTP setup before they can access the admin control plane." title="Admin TOTP bootstrap" tone="warning" />
      )}
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>Bootstrap Admin TOTP</CardTitle>
            <CardDescription>Use the verified admin email and password to create the first authenticator secret.</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/admin-bootstrap" method="post">
              <Field label="Admin email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field label="Password">
                <Input autoComplete="current-password" name="password" required type="password" />
              </Field>
              <ButtonRow>
                <Button type="submit">Create TOTP secret</Button>
                <Link className="button button--ghost" href="/login">
                  Back to login
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            This path is only for configured admin accounts that have already verified their email.
          </CardFooter>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Authenticator details</CardTitle>
            <CardDescription>Keep this secret private. Use the current code immediately on the login page.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Admin email: {bootstrapEmail || "-"}</li>
              <li>TOTP secret: {secret || "Generate it first"}</li>
              <li>Current TOTP code: {code || "Generate it first"}</li>
            </ul>
          </CardBody>
          <CardFooter>
            <Link href={`/login?email=${encodeURIComponent(bootstrapEmail || email)}&totp=1`}>Continue to login with TOTP</Link>
          </CardFooter>
        </Card>
      </div>
    </>
  );
}
