import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
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
  admin_totp_required?: boolean;
  email?: string;
  email_verified?: boolean;
  totp_enabled?: boolean;
};

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const PENDING_TOTP_SECRET_COOKIE = "pending_totp_secret";
const PENDING_TOTP_CODE_COOKIE = "pending_totp_code";
const TOTP_ISSUER = "Grid.Binance";

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function SecurityPage({ params, searchParams }: SecurityPageProps) {
  const { locale } = await params;
  const resolved = (await searchParams) ?? {};
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const error = firstValue(resolved.error);
  const security = firstValue(resolved.security);
  const profile = await fetchProfile();
  const secret = cookieStore.get(PENDING_TOTP_SECRET_COOKIE)?.value ?? "";
  const code = cookieStore.get(PENDING_TOTP_CODE_COOKIE)?.value ?? "";
  const provisioningUri = buildTotpProvisioningUri(profile?.email ?? "", secret);

  return (
    <>
      <StatusBanner
        description={pickText(lang, "密码与 TOTP 操作直接走后端安全接口，只有后端接受后才显示成功。", "Password and TOTP actions are wired to real backend security endpoints and only show success after backend acceptance.")}
        title={pickText(lang, "安全状态条", "Security status strip")}
       
      />
      {error ? <StatusBanner description={error} title={pickText(lang, "安全操作失败", "Security action failed")} /> : null}
      {security === "totp-enabled" ? (
        <StatusBanner
          description={pickText(lang, "请把 TOTP 密钥保存到验证器，并在下次登录挑战时使用当前验证码。", "Store the TOTP secret in your authenticator app and use the current code during the next login challenge.")}
          title={pickText(lang, "TOTP 已启用", "TOTP enabled")}
         
        />
      ) : null}
      <AppShellSection
        description={pickText(lang, "主面板处理密码与 TOTP 操作，侧栏只展示关键安全姿态。", "The main panel handles password and TOTP actions while the side panel keeps critical posture visible.")}
        eyebrow={pickText(lang, "安全中心", "Security center")}
        title={pickText(lang, "安全中心", "Security Center")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "凭证操作", "Credential operations")}</CardTitle>
              <CardDescription>{pickText(lang, "修改密码需要当前密码，成功后会撤销当前会话。", "Password changes require the current password and revoke the active session on success.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <FormStack action="/api/user/security" method="post">
                <Field hint={pickText(lang, "这是后端改密接口必填项。", "Required by the backend password change endpoint.")} label={pickText(lang, "当前密码", "Current password")}>
                  <Input name="currentPassword" required type="password" />
                </Field>
                <Field hint={pickText(lang, "建议先换强密码，再开启 TOTP。", "Use a unique password before enabling TOTP.")} label={pickText(lang, "新密码", "New password")}>
                  <Input name="password" required type="password" />
                </Field>
                <Button name="intent" type="submit" value="password">
                  {pickText(lang, "更新密码", "Update password")}
                </Button>
              </FormStack>
              <div className="flex items-center gap-2">
                <FormStack action="/api/user/security" method="post">
                  <Button name="intent" type="submit" value="enable-totp">
                    {pickText(lang, "启用 TOTP", "Enable TOTP")}
                  </Button>
                </FormStack>
                <FormStack action="/api/user/security" method="post">
                  <Button name="intent" type="submit" value="disable-totp">
                    {pickText(lang, "停用 TOTP", "Disable TOTP")}
                  </Button>
                </FormStack>
              </div>
              <p>{pickText(lang, "密码修改成功或停用 TOTP 后，当前会话会失效并回到登录页。", "Successful password changes and TOTP disable actions revoke the session and return you to login.")}</p>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "安全检查点", "Security checkpoints")}</CardTitle>
              <CardDescription>{pickText(lang, "这些关键姿态值来自后端 profile 接口，而不是前端本地状态。", "Critical posture values come from the backend profile endpoint, not local product state.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <div className="chip-row">
                <Chip tone={profile?.email_verified ? "success" : "warning"}>
                  {pickText(lang, "邮箱", "Email")}: {profile?.email_verified ? pickText(lang, "已验证", "Verified") : pickText(lang, "未验证", "Unverified")}
                </Chip>
                <Chip tone={profile?.totp_enabled ? "success" : "warning"}>
                  TOTP: {profile?.totp_enabled ? pickText(lang, "已启用", "Enabled") : pickText(lang, "未启用", "Disabled")}
                </Chip>
                <Chip tone={profile?.admin_totp_required ? "warning" : "info"}>
                  {pickText(lang, "管理员 TOTP 要求", "Admin TOTP required")}: {profile?.admin_totp_required ? pickText(lang, "是", "Yes") : pickText(lang, "否", "No")}
                </Chip>
              </div>
              {security === "totp-enabled" ? (
                <div className="ui-form">
                  <Field hint={pickText(lang, "这是标准 Base32 密钥，可手动添加到验证器。", "This is a standard Base32 secret for manual authenticator setup.")} label={pickText(lang, "TOTP 密钥", "TOTP secret")}>
                    <Input readOnly value={secret} />
                  </Field>
                  <Field hint={pickText(lang, "支持 otpauth:// 的密码管理器或验证器可以直接导入。", "Password managers or authenticators that support otpauth:// can import this directly.")} label={pickText(lang, "TOTP 导入链接", "TOTP provisioning URI")}>
                    <Input readOnly value={provisioningUri} />
                  </Field>
                  <Field hint={pickText(lang, "首次 TOTP 登录时会用到这个当前验证码。", "Use this current code to verify the first TOTP-based login.")} label={pickText(lang, "当前 TOTP 验证码", "Current TOTP code")}>
                    <Input readOnly value={code} />
                  </Field>
                </div>
              ) : null}
              <ul className="text-list">
                <li>{pickText(lang, "账户邮箱", "Account email")}: {profile?.email ?? pickText(lang, "不可用", "Unavailable")}</li>
                <li>{pickText(lang, "管理员账号在 V1 必须启用 TOTP。", "Admin accounts must use TOTP in V1.")}</li>
                <li>{pickText(lang, "Binance 密钥保存后仍保持掩码显示。", "Binance secrets remain masked even after save.")}</li>
                <li>{pickText(lang, "会话撤销由后端密码与 TOTP 生命周期动作强制执行。", "Session revocation is enforced by backend password and TOTP lifecycle actions.")}</li>
              </ul>
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}


function buildTotpProvisioningUri(email: string, secret: string) {
  if (!email || !secret) {
    return "";
  }

  const label = `${TOTP_ISSUER}:${email}`;
  return `otpauth://totp/${encodeURIComponent(label)}?secret=${encodeURIComponent(secret)}&issuer=${encodeURIComponent(TOTP_ISSUER)}&algorithm=SHA1&digits=6&period=30`;
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
