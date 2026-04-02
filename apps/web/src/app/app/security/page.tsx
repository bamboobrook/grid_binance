import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

type SecurityPageProps = {
  searchParams?: Promise<{
    error?: string | string[];
  }>;
};

export default async function SecurityPage({ searchParams }: SecurityPageProps) {
  const state = await getCurrentUserProductState();
  const error = Array.isArray((await searchParams)?.error)
    ? (await searchParams)?.error?.[0]
    : (await searchParams)?.error;

  return (
    <>
      <StatusBanner
        description="Password and TOTP actions are bridged to the real backend security endpoints. Success is shown only after the backend accepts the action."
        title="Security center"
        tone="info"
      />
      {error ? <StatusBanner description={error} title="Security action failed" tone="warning" /> : null}
      {state.flash.security === "TOTP enabled" ? (
        <StatusBanner
          description="TOTP is now enabled for future login challenges."
          title="TOTP enabled"
          tone="success"
        />
      ) : null}
      <AppShellSection
        description="Protect account access with password rotation, TOTP enablement, TOTP disablement, and an honest session review surface."
        eyebrow="Security center"
        title="Security Center"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Credential operations</CardTitle>
              <CardDescription>Password change requires the current password and revokes the current session on success.</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/security" method="post">
                <Field hint="Required for the real backend password change endpoint." label="Current password">
                  <Input name="currentPassword" required type="password" />
                </Field>
                <Field hint="Use a unique password before enabling TOTP." label="New password">
                  <Input name="password" required type="password" />
                </Field>
                <Button name="intent" type="submit" value="password">
                  Update password
                </Button>
              </FormStack>
              <div className="button-row">
                <FormStack action="/api/user/security" method="post">
                  <Button name="intent" type="submit" value="enable-totp">
                    Enable TOTP
                  </Button>
                </FormStack>
                <FormStack action="/api/user/security" method="post">
                  <Button name="intent" tone="secondary" type="submit" value="disable-totp">
                    Disable TOTP
                  </Button>
                </FormStack>
              </div>
              <p>Successful password changes and TOTP disable actions revoke the current session and send you back to login.</p>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>Security checkpoints</CardTitle>
              <CardDescription>Current posture remains visible; session review is read-only in this build.</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="chip-row">
                <Chip tone="success">Email: Verified</Chip>
                <Chip tone={state.security.totpEnabled === null ? "info" : state.security.totpEnabled ? "success" : "warning"}>
                  TOTP: {state.security.totpEnabled === null ? "Unknown" : state.security.totpEnabled ? "Enabled" : "Disabled"}
                </Chip>
                <Chip tone="info">Session review: Read-only</Chip>
              </div>
              <ul className="text-list">
                <li>Password changed at: {state.security.passwordChangedAt ?? "Not recently changed"}</li>
                <li>Admin accounts must use TOTP in V1.</li>
                <li>Binance secrets remain masked even after save.</li>
                <li>Session revocation beyond password/TOTP lifecycle is not exposed as a fake success path.</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
