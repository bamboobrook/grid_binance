import Link from "next/link";
import { cookies } from "next/headers";
import { ShieldAlert } from "lucide-react";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { firstValue } from "@/lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

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

export default async function AdminBootstrapPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const [searchParamsValue, cookieStore] = await Promise.all([searchParams, cookies()]);
  const resolved = searchParamsValue ?? {};
  const email = firstValue(resolved.email) ?? "";
  const error = firstValue(resolved.error);
  const setup = firstValue(resolved.setup) === "ready";
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const secret = cookieStore.get(PENDING_ADMIN_TOTP_SECRET_COOKIE)?.value ?? "";
  const code = cookieStore.get(PENDING_ADMIN_TOTP_CODE_COOKIE)?.value ?? "";
  const bootstrapEmail = cookieStore.get(PENDING_ADMIN_TOTP_EMAIL_COOKIE)?.value ?? email;

  return (
    <div className="w-full max-w-[500px] space-y-6">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-bold tracking-tight text-white">
          {pickText(lang, "管理员 TOTP 初始化", "Admin TOTP Bootstrap")}
        </h1>
        <p className="text-sm text-slate-400">
          {pickText(lang, "这是首次安全配置，仅限管理员账号使用。", "This is a one-time security setup required for admin accounts.")}
        </p>
      </div>

      {error ? (
        <StatusBanner description={error} title={pickText(lang, "管理员 TOTP 初始化失败", "Admin TOTP bootstrap failed")} tone="danger" />
      ) : setup && secret ? (
        <StatusBanner description={pickText(lang, "请把密钥保存到验证器应用中，再用页面显示的当前验证码完成首次管理员登录。", "Store the secret in your authenticator app, then use the shown code to complete the first admin login.")} title={pickText(lang, "管理员 TOTP 已就绪", "Admin TOTP ready")} tone="success" />
      ) : (
        <StatusBanner description={pickText(lang, "已配置的管理员账号必须先完成 TOTP 初始化，才能进入管理后台。", "Configured admin accounts must complete TOTP setup before they can access the admin control plane.")} title={pickText(lang, "初始化要求", "Bootstrap Required")} tone="info" />
      )}

      {!setup ? (
        <Card className="bg-[#131b2c] border-slate-800 shadow-xl">
          <CardHeader className="border-b border-slate-800/60 pb-4">
            <CardTitle>{pickText(lang, "初始化管理员 TOTP", "Bootstrap Admin TOTP")}</CardTitle>
            <CardDescription>{pickText(lang, "使用已验证的管理员邮箱和密码，创建首次验证器密钥。", "Use the verified admin email and password to create the first authenticator secret.")}</CardDescription>
          </CardHeader>
          <CardBody className="p-6">
            <FormStack action={`/api/auth/admin-bootstrap?locale=${locale}`} method="post" className="space-y-5">
              <Field label={pickText(lang, "管理员邮箱", "Admin email")}>
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" className="bg-slate-900 border-slate-700 h-10 text-sm" />
              </Field>
              <Field label={pickText(lang, "密码", "Password")}>
                <Input autoComplete="current-password" name="password" required type="password" className="bg-slate-900 border-slate-700 h-10 text-sm" />
              </Field>
              <Button type="submit" tone="primary" className="w-full h-11 text-sm font-bold shadow-lg shadow-primary/20">
                <ShieldAlert className="w-4 h-4 mr-2" />
                {pickText(lang, "创建 TOTP 密钥", "Create TOTP secret")}
              </Button>
            </FormStack>
          </CardBody>
          <div className="border-t border-slate-800/60 bg-slate-800/30 p-4 text-center flex flex-col gap-2">
            <Link href={`/${locale}/login`} className="text-xs text-slate-400 hover:text-white transition-colors">
              {pickText(lang, "返回登录", "Back to login")}
            </Link>
          </div>
        </Card>
      ) : (
        <Card className="bg-[#131b2c] border-slate-800 shadow-xl">
          <CardHeader className="border-b border-slate-800/60 pb-4">
            <CardTitle className="text-emerald-500">{pickText(lang, "验证器信息", "Authenticator details")}</CardTitle>
            <CardDescription>{pickText(lang, "请妥善保管密钥，并立即用当前验证码完成登录。", "Keep this secret private. Use the current code immediately on the login page.")}</CardDescription>
          </CardHeader>
          <CardBody className="p-6">
            <div className="space-y-4">
              <div className="flex flex-col gap-1">
                <span className="text-xs font-bold text-slate-500 uppercase">{pickText(lang, "管理员邮箱", "Admin email")}</span>
                <span className="text-sm font-medium text-slate-300">{bootstrapEmail || "-"}</span>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-xs font-bold text-slate-500 uppercase">{pickText(lang, "TOTP 密钥", "TOTP secret")}</span>
                <span className="text-sm font-mono text-emerald-400 tracking-wider bg-emerald-500/10 p-2 rounded border border-emerald-500/20 break-all">{secret || "-"}</span>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-xs font-bold text-slate-500 uppercase">{pickText(lang, "当前验证码", "Current TOTP code")}</span>
                <span className="text-xl font-mono text-slate-300 tracking-widest">{code || "-"}</span>
              </div>
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
