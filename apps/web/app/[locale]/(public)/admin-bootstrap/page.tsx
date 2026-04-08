import Link from "next/link";
import { cookies } from "next/headers";
import { ShieldAlert } from "lucide-react";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { firstValue } from "@/lib/auth";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    setup?: string | string[];
  }>;
};

const PENDING_ADMIN_TOTP_SECRET_COOKIE = "pending_admin_totp_secret";
const PENDING_ADMIN_TOTP_CODE_COOKIE = "pending_admin_totp_code";
const PENDING_ADMIN_TOTP_EMAIL_COOKIE = "pending_admin_totp_email";
const TOTP_ISSUER = "Grid.Binance";

export default async function AdminBootstrapPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const [searchParamsValue, cookieStore] = await Promise.all([searchParams, cookies()]);
  const resolved = searchParamsValue ?? {};
  const email = firstValue(resolved.email) ?? "";
  const error = firstValue(resolved.error);
  const setup = firstValue(resolved.setup) === "ready";
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const secret = cookieStore.get(PENDING_ADMIN_TOTP_SECRET_COOKIE)?.value ?? "";
  const code = cookieStore.get(PENDING_ADMIN_TOTP_CODE_COOKIE)?.value ?? "";
  const bootstrapEmail = cookieStore.get(PENDING_ADMIN_TOTP_EMAIL_COOKIE)?.value ?? email;
  const provisioningUri = buildTotpProvisioningUri(bootstrapEmail || email, secret);

  return (
    <div className="w-full max-w-[500px] space-y-6">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-bold tracking-tight text-foreground">
          {pickText(lang, "管理员 TOTP 初始化", "Admin TOTP Bootstrap")}
        </h1>
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "这是首次安全配置，仅限管理员账号使用。", "This is a one-time security setup required for admin accounts.")}
        </p>
      </div>

      {error ? (
        <StatusBanner description={error} title={pickText(lang, "管理员 TOTP 初始化失败", "Admin TOTP bootstrap failed")} tone="danger" />
      ) : setup && secret ? (
        <StatusBanner
          description={pickText(lang, "请把 Base32 密钥保存到验证器；如果密码管理器支持，也可以直接导入下方的 otpauth 链接。", "Save the Base32 secret in your authenticator app, or import the otpauth link below if your password manager supports it.")}
          title={pickText(lang, "管理员 TOTP 已就绪", "Admin TOTP ready")}
          tone="success"
        />
      ) : (
        <StatusBanner description={pickText(lang, "已配置的管理员账号必须先完成 TOTP 初始化，才能进入管理后台。", "Configured admin accounts must complete TOTP setup before they can access the admin control plane.")} title={pickText(lang, "初始化要求", "Bootstrap Required")} tone="info" />
      )}

      {!setup ? (
        <Card className="bg-card border-border shadow-xl">
          <CardHeader className="border-b border-border/60 pb-4">
            <CardTitle>{pickText(lang, "初始化管理员 TOTP", "Bootstrap Admin TOTP")}</CardTitle>
            <CardDescription>{pickText(lang, "使用已验证的管理员邮箱和密码，创建首次验证器密钥。", "Use the verified admin email and password to create the first authenticator secret.")}</CardDescription>
          </CardHeader>
          <CardBody className="p-6">
            <FormStack action={`/api/auth/admin-bootstrap?locale=${locale}`} method="post" className="space-y-5">
              <Field label={pickText(lang, "管理员邮箱", "Admin email")}>
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" className="bg-input border-border h-10 text-sm" />
              </Field>
              <Field label={pickText(lang, "密码", "Password")}>
                <Input autoComplete="current-password" name="password" required type="password" className="bg-input border-border h-10 text-sm" />
              </Field>
              <Button type="submit" tone="primary" className="w-full h-11 text-sm font-bold shadow-lg shadow-primary/20">
                <ShieldAlert className="w-4 h-4 mr-2" />
                {pickText(lang, "创建 TOTP 密钥", "Create TOTP secret")}
              </Button>
            </FormStack>
          </CardBody>
          <div className="border-t border-border/60 bg-secondary/30 p-4 text-center flex flex-col gap-2">
            <Link href={`/${locale}/login`} className="text-xs text-muted-foreground hover:text-foreground transition-colors">
              {pickText(lang, "返回登录", "Back to login")}
            </Link>
          </div>
        </Card>
      ) : (
        <Card className="bg-card border-border shadow-xl">
          <CardHeader className="border-b border-border/60 pb-4">
            <CardTitle className="text-emerald-500">{pickText(lang, "验证器信息", "Authenticator details")}</CardTitle>
            <CardDescription>{pickText(lang, "请妥善保管密钥，并立即用当前验证码完成登录。", "Keep this secret private. Use the current code immediately on the login page.")}</CardDescription>
          </CardHeader>
          <CardBody className="p-6">
            <div className="space-y-4">
              <Field label={pickText(lang, "管理员邮箱", "Admin email")}>
                <Input readOnly value={bootstrapEmail || "-"} />
              </Field>
              <Field label={pickText(lang, "TOTP 密钥", "TOTP secret")} hint={pickText(lang, "这是标准 Base32 密钥，可手动添加到验证器。", "This is a standard Base32 secret for manual authenticator setup.")}>
                <Input readOnly value={secret || "-"} />
              </Field>
              <Field label={pickText(lang, "TOTP 导入链接", "TOTP provisioning URI")} hint={pickText(lang, "支持 otpauth:// 的密码管理器或验证器可以直接导入。", "Password managers or authenticators that support otpauth:// can import this directly.")}>
                <Input readOnly value={provisioningUri || "-"} />
              </Field>
              <Field label={pickText(lang, "当前验证码", "Current TOTP code")}>
                <Input readOnly value={code || "-"} />
              </Field>
              <Link href={`/${locale}/login?email=${encodeURIComponent(bootstrapEmail || email)}&totp=1`} className="block pt-2">
                <Button tone="primary" className="w-full h-11 text-sm font-bold shadow-lg shadow-primary/20">
                  {pickText(lang, "带 TOTP 返回登录", "Continue to login with TOTP")}
                </Button>
              </Link>
            </div>
          </CardBody>
        </Card>
      )}
    </div>
  );
}

function buildTotpProvisioningUri(email: string, secret: string) {
  if (!email || !secret) {
    return "";
  }

  const label = `${TOTP_ISSUER}:${email}`;
  return `otpauth://totp/${encodeURIComponent(label)}?secret=${encodeURIComponent(secret)}&issuer=${encodeURIComponent(TOTP_ISSUER)}&algorithm=SHA1&digits=6&period=30`;
}
