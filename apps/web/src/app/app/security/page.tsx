import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getSecuritySnapshot } from "../../../lib/api/server";

export default async function SecurityPage() {
  const snapshot = await getSecuritySnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="This shared form surface will later connect password reset, device review, and TOTP lifecycle endpoints."
        eyebrow="Security center"
        title="Security"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Credentials</CardTitle>
              <CardDescription>Passwords and TOTP stay under one reusable form contract.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="#" method="post">
                <Field hint="Use a strong password before enabling app-level TOTP." label="New password">
                  <Input name="password" type="password" />
                </Field>
                <Field hint="The current build only demonstrates the shared UI shell." label="TOTP verification code">
                  <Input inputMode="numeric" name="totp" pattern="[0-9]{6}" />
                </Field>
                <ButtonRow>
                  <Button type="submit">Save security changes</Button>
                </ButtonRow>
              </FormStack>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Security checkpoints</CardTitle>
              <CardDescription>Current account posture.</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="chip-row">
                {snapshot.checkpoints.map((item) => (
                  <Chip key={item.label} tone="success">
                    {item.label}: {item.value}
                  </Chip>
                ))}
              </div>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
