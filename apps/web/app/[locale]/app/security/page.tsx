import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute } from "@/lib/ui/preferences";

type SecurityPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    error?: string | string[];
    security?: string | string[];
  }>;
};

type ProfileResponse = {
  totp_enabled?: boolean;
};

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const PENDING_TOTP_SECRET_COOKIE = "pending_totp_secret";
const PENDING_TOTP_CODE_COOKIE = "pending_totp_code";

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function SecurityPage({ params, searchParams }: SecurityPageProps) {
  const { locale } = await params;
  const query = (await searchParams) ?? {};
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const error = firstValue(query.error);
  const security = firstValue(query.security);
  const profile = await fetchProfile();
  const secret = cookieStore.get(PENDING_TOTP_SECRET_COOKIE)?.value ?? "";
  const code = cookieStore.get(PENDING_TOTP_CODE_COOKIE)?.value ?? "";
  const twoStepEnabled = profile?.totp_enabled === true || security === "totp-enabled";

  return (
    <>
      {error ? <StatusBanner description={error} title={pickText(lang, "安全操作失败", "Security action failed")}  tone="info" lang={lang} /> : null}
      {security === "totp-enabled" ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={pickText(lang, "请把下面的密钥保存到验证器应用里，下次登录时会用到。", "Save the secret below in your authenticator app. You will use it the next time you sign in.")}
          title={pickText(lang, "两步验证已启用", "Two-step verification enabled")}
         
        />
      ) : null}
      <AppShellSection
        eyebrow={pickText(lang, "安全中心", "Security center")}
        title={pickText(lang, "安全中心", "Security Center")}
      >
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "修改密码", "Change password")}</CardTitle>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/security" method="post">
                <Field label={pickText(lang, "当前密码", "Current password")}>
                  <Input autoComplete="current-password" name="currentPassword" required type="password" />
                </Field>
                <Field label={pickText(lang, "新密码", "New password")}>
                  <Input autoComplete="new-password" name="password" required type="password" />
                </Field>
                <Field label={pickText(lang, "确认新密码", "Confirm new password")}>
                  <Input autoComplete="new-password" name="confirmPassword" required type="password" />
                </Field>
                <Button name="intent" type="submit" value="password">
                  {pickText(lang, "更新密码", "Update password")}
                </Button>
              </FormStack>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "两步验证", "Two-step verification")}</CardTitle>
              <CardDescription>{pickText(lang, "开启后，登录时需要输入验证器里的 6 位验证码。", "After enabling it, sign-in requires a 6-digit code from your authenticator app.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="mb-4 rounded-sm border border-border bg-secondary/30 px-3 py-2">
                <span className="text-xs font-bold text-muted-foreground uppercase tracking-wider">
                  {pickText(lang, "当前状态", "Current status")}
                </span>
                <p className="mt-1 text-sm font-semibold text-foreground">
                  {twoStepEnabled ? pickText(lang, "已启用", "Enabled") : pickText(lang, "未启用", "Disabled")}
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <FormStack action="/api/user/security" method="post">
                  <Button disabled={twoStepEnabled} name="intent" type="submit" value="enable-totp">
                    {pickText(lang, "启用两步验证", "Enable two-step verification")}
                  </Button>
                </FormStack>
                <FormStack action="/api/user/security" method="post">
                  <Button disabled={!twoStepEnabled} name="intent" type="submit" value="disable-totp" tone="outline">
                    {pickText(lang, "停用两步验证", "Disable two-step verification")}
                  </Button>
                </FormStack>
              </div>
              {security === "totp-enabled" ? (
                <div className="ui-form mt-4 rounded-sm border border-border bg-secondary/30 p-3">
                  <Field hint={pickText(lang, "先保存到验证器应用。", "Save this in your authenticator app.")} label={pickText(lang, "验证器密钥", "Authenticator secret")}>
                    <Input readOnly value={secret} />
                  </Field>
                  <Field hint={pickText(lang, "首次登录时可用。", "Use this for the first sign-in.")} label={pickText(lang, "当前验证码", "Current code")}>
                    <Input readOnly value={code} />
                  </Field>
                </div>
              ) : null}
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
  if (sessionToken === "") {
    return null;
  }
  const response = await fetch(authApiBaseUrl() + "/profile", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
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
