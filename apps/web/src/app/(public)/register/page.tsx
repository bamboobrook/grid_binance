import Link from "next/link";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { Tabs } from "../../../components/ui/tabs";
import { getPublicAuthSnapshot } from "../../../lib/api/server";
import { firstValue, safeRedirectTarget } from "../../../lib/auth";

type RegisterPageProps = {
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
  }>;
};

const onboardingNotes = [
  "Email verification is required before normal sign-in is allowed.",
  "One Binance account per user keeps exchange ownership explicit.",
  "Membership is required before any strategy can start.",
];

export default async function RegisterPage({ searchParams }: RegisterPageProps) {
  const snapshot = await getPublicAuthSnapshot("register");
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const next = safeRedirectTarget(firstValue(params.next), "/app/dashboard");

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
        <StatusBanner description={error} title="Registration failed" tone="danger" />
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
            <FormStack action="/api/auth/register" method="post">
              <input name="next" type="hidden" value={next} />
              <Field hint="Email verification is required before sign-in is allowed." label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field hint="Use a unique password before enabling TOTP in the security center." label="Password">
                <Input autoComplete="new-password" name="password" required type="password" />
              </Field>
              <div className="chip-row">
                {snapshot.checklist.map((item) => (
                  <Chip key={item} tone="warning">
                    {item}
                  </Chip>
                ))}
              </div>
              <ButtonRow>
                <Button type="submit">{snapshot.submitLabel}</Button>
                <Link className="button button--ghost" href="/help/expiry-reminder">
                  Billing help
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href={snapshot.alternateHref}>{snapshot.alternateLabel}</Link>
          </CardFooter>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Account onboarding</CardTitle>
            <CardDescription>Registration moves directly into the documented user lifecycle.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {onboardingNotes.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
