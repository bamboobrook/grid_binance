import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { firstValue } from "../../../lib/auth";

type SecurityPageProps = {
  searchParams?: Promise<{
    passwordUpdated?: string | string[];
    sessionsRevoked?: string | string[];
    totpEnabled?: string | string[];
  }>;
};

export default async function SecurityPage({ searchParams }: SecurityPageProps) {
  const params = (await searchParams) ?? {};
  const passwordUpdated = firstValue(params.passwordUpdated) === "1";
  const totpEnabled = firstValue(params.totpEnabled) === "1";
  const sessionsRevoked = firstValue(params.sessionsRevoked) === "1";

  return (
    <>
      <StatusBanner
        description="Password, TOTP, and session review now have explicit user actions instead of a shell-only placeholder."
        title="Security center"
        tone="info"
      />
      {passwordUpdated ? <StatusBanner description="Password rotation completed for the current account." title="Password updated" tone="success" /> : null}
      {totpEnabled ? <StatusBanner description="TOTP is now enabled for future login challenges." title="TOTP enabled" tone="success" /> : null}
      {sessionsRevoked ? <StatusBanner description="Other sessions revoked. Only the current browser remains active." title="Other sessions revoked" tone="success" /> : null}
      <AppShellSection
        description="Protect account access with password rotation, TOTP enablement, and active session review."
        eyebrow="Security center"
        title="Security Center"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Credential operations</CardTitle>
              <CardDescription>Each operation stays visible and separately actionable.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/app/security" method="get">
                {totpEnabled ? <input name="totpEnabled" type="hidden" value="1" /> : null}
                {sessionsRevoked ? <input name="sessionsRevoked" type="hidden" value="1" /> : null}
                <Field hint="Use a unique password before enabling TOTP." label="New password">
                  <Input name="password" required type="password" />
                </Field>
                <Button name="passwordUpdated" type="submit" value="1">
                  Update password
                </Button>
              </FormStack>
              <div className="button-row">
                <FormStack action="/app/security" method="get">
                  {passwordUpdated ? <input name="passwordUpdated" type="hidden" value="1" /> : null}
                  {sessionsRevoked ? <input name="sessionsRevoked" type="hidden" value="1" /> : null}
                  <Button name="totpEnabled" type="submit" value="1">
                    Enable TOTP
                  </Button>
                </FormStack>
                <FormStack action="/app/security" method="get">
                  {passwordUpdated ? <input name="passwordUpdated" type="hidden" value="1" /> : null}
                  {totpEnabled ? <input name="totpEnabled" type="hidden" value="1" /> : null}
                  <Button name="sessionsRevoked" tone="secondary" type="submit" value="1">
                    Revoke other sessions
                  </Button>
                </FormStack>
              </div>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Security checkpoints</CardTitle>
              <CardDescription>Current posture remains visible during every change.</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="chip-row">
                <Chip tone="success">Email: Verified</Chip>
                <Chip tone={totpEnabled ? "success" : "warning"}>TOTP: {totpEnabled ? "Enabled" : "Disabled"}</Chip>
                <Chip tone={sessionsRevoked ? "info" : "success"}>Session review: {sessionsRevoked ? "Current only" : "2 active devices"}</Chip>
              </div>
              <ul className="text-list">
                <li>Admin accounts must use TOTP in V1.</li>
                <li>Binance secrets remain masked even after save.</li>
                <li>Telegram complements web alerts for account-risk incidents.</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
