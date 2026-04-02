import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

export default async function SecurityPage() {
  const state = await getCurrentUserProductState();

  return (
    <>
      <StatusBanner
        description="Password, TOTP, and session review now have explicit POST actions instead of URL-driven toggles."
        title="Security center"
        tone="info"
      />
      {state.flash.security ? (
        <StatusBanner
          description={
            state.flash.security === "TOTP disabled"
              ? "TOTP has been disabled. Password login remains active until re-enabled."
              : state.flash.security === "TOTP enabled"
                ? "TOTP is now enabled for future login challenges."
                : state.flash.security === "Other sessions revoked"
                  ? "Only the current browser remains active."
                  : "Password rotation completed for the current account."
          }
          title={state.flash.security}
          tone="success"
        />
      ) : null}
      <AppShellSection
        description="Protect account access with password rotation, TOTP enablement, TOTP disablement, and active session review."
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
              <FormStack action="/api/user/security" method="post">
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
                <FormStack action="/api/user/security" method="post">
                  <Button name="intent" tone="secondary" type="submit" value="revoke-sessions">
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
                <Chip tone={state.security.totpEnabled ? "success" : "warning"}>
                  TOTP: {state.security.totpEnabled ? "Enabled" : "Disabled"}
                </Chip>
                <Chip tone={state.security.sessionsRevokedAt ? "info" : "success"}>
                  Session review: {state.security.sessionsRevokedAt ? "Current only" : "2 active devices"}
                </Chip>
              </div>
              <ul className="text-list">
                <li>Password changed at: {state.security.passwordChangedAt ?? "Not recently changed"}</li>
                <li>Sessions revoked at: {state.security.sessionsRevokedAt ?? "No recent revocation"}</li>
                <li>Admin accounts must use TOTP in V1.</li>
                <li>Binance secrets remain masked even after save.</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
