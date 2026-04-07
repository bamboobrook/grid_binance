import Link from "next/link";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { Tabs } from "../../../components/ui/tabs";
import { firstValue, safeRedirectTarget } from "../../../lib/auth";

type VerifyEmailPageProps = {
  searchParams?: Promise<{
    code?: string | string[];
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
    notice?: string | string[];
  }>;
};

export default async function VerifyEmailPage({ searchParams }: VerifyEmailPageProps) {
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const code = firstValue(params.code) ?? "";
  const error = firstValue(params.error);
  const next = safeRedirectTarget(firstValue(params.next), "/app/dashboard");
  const notice = firstValue(params.notice);

  return (
    <>
      <Tabs
        activeHref="/register"
        items={[
          { href: "/login", label: "Login" },
          { href: "/register", label: "Register" },
        ]}
        label="Authentication pages"
      />
      {error ? (
        <StatusBanner description={error} title="Email verification failed" tone="danger" />
      ) : (
        <StatusBanner
          description={notice === "registration-created" ? "Check your email for the issued verification code before your first login." : "Verification must complete before login is allowed. The code is sent to your email address."}
          title="Verify Email"
          tone="warning"
        />
      )}
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>Verify Email</CardTitle>
            <CardDescription>Confirm the verification code delivered to the email address used during registration.</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/verify-email" method="post">
              <input name="next" type="hidden" value={next} />
              <Field label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field hint="Enter the verification code delivered to your email inbox before the first login." label="Verification code">
                <Input inputMode="numeric" name="code" required pattern="[0-9]{6}" />
              </Field>
              <ButtonRow>
                <Button type="submit">Verify email</Button>
                <Link className="button button--ghost" href="/login">
                  Back to login
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href="/register">Need to change email? Register again</Link>
          </CardFooter>
        </Card>
      </div>
    </>
  );
}
