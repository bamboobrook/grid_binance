import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { Button, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";

type SecurityPageProps = {
  searchParams?: Promise<{
    error?: string | string[];
    security?: string | string[];
  }>;
};

type ProfileResponse = {
  admin_totp_required?: boolean;
  email?: string;
  email_verified?: boolean;
  totp_enabled?: boolean;
};

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const PENDING_TOTP_SECRET_COOKIE = "pending_totp_secret";
const PENDING_TOTP_CODE_COOKIE = "pending_totp_code";

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function SecurityPage({ searchParams }: SecurityPageProps) {
  const params = (await searchParams) ?? {};
  const cookieStore = await cookies();
  const error = firstValue(params.error);
  const security = firstValue(params.security);
  const profile = await fetchProfile();
  const secret = cookieStore.get(PENDING_TOTP_SECRET_COOKIE)?.value ?? "";
  const code = cookieStore.get(PENDING_TOTP_CODE_COOKIE)?.value ?? "";

  return (
    <>
      <StatusBanner
        description="Password and TOTP actions are bridged to the real backend security endpoints. Success is shown only after the backend accepts the action."
        title="Security center"
        tone="info"
      />
      {error ? <StatusBanner description={error} title="Security action failed" tone="warning" /> : null}
      {security === "totp-enabled" ? (
        <StatusBanner
          description="Store the TOTP secret in your authenticator app and use the current code on the next login challenge."
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
              <CardDescription>Critical posture values come from the backend profile endpoint instead of local product state.</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="chip-row">
                <Chip tone={profile?.email_verified ? "success" : "warning"}>
                  Email: {profile?.email_verified ? "Verified" : "Unverified"}
                </Chip>
                <Chip tone={profile?.totp_enabled ? "success" : "warning"}>
                  TOTP: {profile?.totp_enabled ? "Enabled" : "Disabled"}
                </Chip>
                <Chip tone={profile?.admin_totp_required ? "warning" : "info"}>
                  Admin TOTP required: {profile?.admin_totp_required ? "Yes" : "No"}
                </Chip>
              </div>
              {security === "totp-enabled" ? (
                <div className="ui-form">
                  <Field hint="Save this in your authenticator app before the next login." label="TOTP secret">
                    <Input readOnly value={secret} />
                  </Field>
                  <Field hint="Use this current code to verify the first TOTP-based login." label="Current TOTP code">
                    <Input readOnly value={code} />
                  </Field>
                </div>
              ) : null}
              <ul className="text-list">
                <li>Account email: {profile?.email ?? "Unavailable"}</li>
                <li>Admin accounts must use TOTP in V1.</li>
                <li>Binance secrets remain masked even after save.</li>
                <li>Session revocation is enforced by backend password and TOTP lifecycle actions.</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}

async function fetchProfile(): Promise<ProfileResponse | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
  const response = await fetch(`${authApiBaseUrl()}/profile`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as ProfileResponse;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
