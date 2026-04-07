import Link from "next/link";
import { cookies } from "next/headers";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { Tabs } from "@/components/ui/tabs";
import { firstValue } from "@/lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type PageProps = {
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    setup?: string | string[];
  }>;
};

const PENDING_ADMIN_TOTP_SECRET_COOKIE = "pending_admin_totp_secret";
const PENDING_ADMIN_TOTP_CODE_COOKIE = "pending_admin_totp_code";
const PENDING_ADMIN_TOTP_EMAIL_COOKIE = "pending_admin_totp_email";

export default async function AdminBootstrapPage({ searchParams }: PageProps) {
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const setup = firstValue(params.setup) === "ready";
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const secret = cookieStore.get(PENDING_ADMIN_TOTP_SECRET_COOKIE)?.value ?? "";
  const code = cookieStore.get(PENDING_ADMIN_TOTP_CODE_COOKIE)?.value ?? "";
  const bootstrapEmail = cookieStore.get(PENDING_ADMIN_TOTP_EMAIL_COOKIE)?.value ?? email;

  return (
    <>
      <Tabs
        activeHref="/admin-bootstrap"
        items={[
          { href: "/login", label: pickText(lang, "登录", "Login") },
          { href: "/register", label: pickText(lang, "注册", "Register") },
          { href: "/admin-bootstrap", label: pickText(lang, "管理员 2FA", "Admin 2FA") },
        ]}
        label={pickText(lang, "认证页面", "Authentication pages")}
      />
      {error ? (
        <StatusBanner description={error} title={pickText(lang, "管理员 TOTP 初始化失败", "Admin TOTP bootstrap failed")} />
      ) : setup && secret ? (
        <StatusBanner description={pickText(lang, "请把密钥保存到验证器应用中，再用页面显示的当前验证码完成首次管理员登录。", "Store the secret in your authenticator app, then use the shown code to complete the first admin login.")} title={pickText(lang, "管理员 TOTP 已就绪", "Admin TOTP ready")} />
      ) : (
        <StatusBanner description={pickText(lang, "已配置的管理员账号必须先完成 TOTP 初始化，才能进入管理后台。", "Configured admin accounts must complete TOTP setup before they can access the admin control plane.")} title={pickText(lang, "初始化管理员 TOTP", "Admin TOTP bootstrap")} />
      )}
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "初始化管理员 TOTP", "Bootstrap Admin TOTP")}</CardTitle>
            <CardDescription>{pickText(lang, "使用已验证的管理员邮箱和密码，创建首次验证器密钥。", "Use the verified admin email and password to create the first authenticator secret.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/admin-bootstrap" method="post">
              <Field label={pickText(lang, "管理员邮箱", "Admin email")}>
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field label={pickText(lang, "密码", "Password")}>
                <Input autoComplete="current-password" name="password" required type="password" />
              </Field>
              <ButtonRow>
                <Button type="submit">{pickText(lang, "创建 TOTP 密钥", "Create TOTP secret")}</Button>
                <Link className="button button--ghost" href="/login">
                  {pickText(lang, "返回登录", "Back to login")}
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            {pickText(lang, "这个入口仅供已验证邮箱、且已被配置为管理员的账号使用。", "This path is only for configured admin accounts that have already verified their email.")}
          </CardFooter>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "验证器信息", "Authenticator details")}</CardTitle>
            <CardDescription>{pickText(lang, "请妥善保管密钥，并立即用当前验证码完成登录。", "Keep this secret private. Use the current code immediately on the login page.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>{pickText(lang, "管理员邮箱", "Admin email")}: {bootstrapEmail || "-"}</li>
              <li>{pickText(lang, "TOTP 密钥", "TOTP secret")}: {secret || pickText(lang, "先生成后再显示", "Generate it first")}</li>
              <li>{pickText(lang, "当前验证码", "Current TOTP code")}: {code || pickText(lang, "先生成后再显示", "Generate it first")}</li>
            </ul>
          </CardBody>
          <CardFooter>
            <Link href={`/login?email=${encodeURIComponent(bootstrapEmail || email)}&totp=1`}>{pickText(lang, "带 TOTP 返回登录", "Continue to login with TOTP")}</Link>
          </CardFooter>
        </Card>
      </div>
    </>
  );
}
