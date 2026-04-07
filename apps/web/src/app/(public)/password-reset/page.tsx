import Link from "next/link";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { Tabs } from "../../../components/ui/tabs";
import { firstValue } from "../../../lib/auth";

type PasswordResetPageProps = {
  searchParams?: Promise<{
    code?: string | string[];
    email?: string | string[];
    error?: string | string[];
    notice?: string | string[];
    step?: string | string[];
  }>;
};

export default async function PasswordResetPage({ searchParams }: PasswordResetPageProps) {
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const code = firstValue(params.code) ?? "";
  const error = firstValue(params.error);
  const notice = firstValue(params.notice);
  const step = firstValue(params.step) === "confirm" ? "confirm" : "request";

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
        <StatusBanner description={error} title="Password reset failed" tone="danger" />
      ) : (
        <StatusBanner
          description={step === "confirm" && notice === "reset-code-issued" ? "Check your email for the issued reset code, then enter it with your new password." : "Request a reset code, then check your email and confirm the new password."}
          title={step === "confirm" && notice === "reset-code-issued" ? "Reset code issued" : step === "confirm" ? "Confirm Password Reset" : "Password Reset"}
          tone="warning"
        />
      )}
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>{step === "confirm" ? "Reset your password" : "Request reset code"}</CardTitle>
            <CardDescription>{step === "confirm" ? "Complete the reset with the code sent to your email inbox." : "Request a password reset code first, then check your email for the code."}</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/password-reset" method="post">
              <input name="intent" type="hidden" value={step} />
              <Field label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              {step === "confirm" ? (
                <>
                  <Field hint="Enter the reset code delivered to your email for this password reset request." label="Reset code">
                    <Input inputMode="numeric" name="code" required pattern="[0-9]{6}" />
                  </Field>
                  <Field label="New password">
                    <Input autoComplete="new-password" name="password" required type="password" />
                  </Field>
                </>
              ) : null}
              <ButtonRow>
                <Button type="submit">{step === "confirm" ? "Reset password" : "Send reset code"}</Button>
                <Link className="button button--ghost" href="/login">
                  Back to login
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href="/register">Need a new account? Register</Link>
          </CardFooter>
        </Card>
      </div>
    </>
  );
}
