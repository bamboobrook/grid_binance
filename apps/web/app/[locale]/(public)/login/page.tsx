import Link from "next/link";
import { cookies } from "next/headers";
import { LogIn } from "lucide-react";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { getPublicAuthSnapshot } from "@/lib/api/server";
import { firstValue, safeRedirectTarget } from "@/lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type LoginPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
    notice?: string | string[];
    security?: string | string[];
    totp?: string | string[];
    adminBootstrap?: string | string[];
  }>;
};

function noticeCopy(lang: "zh" | "en", notice: string | undefined) {
  switch (notice) {
    case "email-verified":
      return {
        title: pickText(lang, "邮箱验证完成", "Email verified"),
        description: pickText(lang, "邮箱已经验证，可以继续用密码登录；如果页面提示，再输入 TOTP。", "Your email is verified. Continue with password login and enter TOTP if prompted."),
      };
    case "password-reset-complete":
      return {
        title: pickText(lang, "密码已重置", "Password reset complete"),
        description: pickText(lang, "请使用新密码重新登录。", "Use the new password to sign in."),
      };
    case "password-updated":
      return {
        title: pickText(lang, "密码已更新", "Password updated"),
        description: pickText(lang, "旧会话已失效，请用新密码重新登录。", "The previous session was revoked. Sign in again with the new password."),
      };
    case "totp-disabled":
      return {
        title: pickText(lang, "TOTP 已关闭", "TOTP disabled"),
        description: pickText(lang, "旧会话已失效，请重新登录。", "The previous session was revoked. Sign in again without a TOTP challenge."),
      };
    default:
      return null;
  }
}

export default async function LoginPage({ params, searchParams }: LoginPageProps) {
  const { locale } = await params;
  const [snapshot, cookieStore] = await Promise.all([getPublicAuthSnapshot("login"), cookies()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const searchParamsValue = (await searchParams) ?? {};
  const email = firstValue(searchParamsValue.email) ?? "";
  const error = firstValue(searchParamsValue.error);
  const next = safeRedirectTarget(firstValue(searchParamsValue.next), "/app/dashboard");
  const notice = noticeCopy(lang, firstValue(searchParamsValue.notice) ?? firstValue(searchParamsValue.security));
  const showTotp = firstValue(searchParamsValue.totp) === "1" || Boolean(error && /totp/i.test(error));
  const showAdminBootstrap = firstValue(searchParamsValue.adminBootstrap) === "1" || Boolean(error && /admin totp setup required/i.test(error ?? ""));

  return (
    <div className="w-full max-w-[420px] space-y-6">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-bold tracking-tight text-foreground">{snapshot.title}</h1>
        <p className="text-sm text-muted-foreground">{snapshot.description}</p>
      </div>

      {error ? (
        <StatusBanner description={error} title={pickText(lang, "登录失败", "Login failed")} tone="danger" />
      ) : notice ? (
        <StatusBanner description={notice.description} title={notice.title} tone="success" />
      ) : snapshot.notice.description ? (
        <StatusBanner description={snapshot.notice.description} title={snapshot.notice.title} tone={snapshot.notice.tone as any} />
      ) : null}

      <Card className="bg-card border-border shadow-xl">
        <CardBody className="p-6">
          <FormStack action={`/api/auth/login?locale=${locale}`} method="post" className="space-y-5">
            <input name="next" type="hidden" value={next} />
            
            <Field label="Email">
              <Input 
                autoComplete="email" 
                defaultValue={email} 
                name="email" 
                required 
                type="email" 
                className="bg-input border-border h-10 text-sm"
                placeholder="name@example.com"
              />
            </Field>

            <Field label={pickText(lang, "密码", "Password")}>
              <Input 
                autoComplete="current-password" 
                name="password" 
                required 
                type="password" 
                className="bg-input border-border h-10 text-sm"
                placeholder="••••••••"
              />
            </Field>

            {showTotp && (
              <Field label="TOTP" hint={pickText(lang, "6 位验证码", "6-digit code")}>
                <Input 
                  autoComplete="one-time-code" 
                  inputMode="numeric" 
                  name="totpCode" 
                  pattern="[0-9]{6}"
                  className="bg-input border-border h-10 font-mono text-center tracking-widest text-lg"
                  placeholder="000000"
                />
              </Field>
            )}

            <Button type="submit" tone="primary" className="w-full h-11 text-sm font-bold shadow-lg shadow-primary/20">
              <LogIn className="w-4 h-4 mr-2" />
              {snapshot.submitLabel}
            </Button>
          </FormStack>
        </CardBody>
        <div className="border-t border-border/60 bg-secondary/30 p-4 text-center flex flex-col gap-2">
          <Link href={`/${locale}/password-reset`} className="text-xs text-muted-foreground hover:text-foreground transition-colors">
            {pickText(lang, "忘记密码？重置密码", "Forgot password? Reset here")}
          </Link>
          <Link href={`/${locale}/register`} className="text-xs text-primary hover:underline font-semibold">
            {snapshot.alternateLabel}
          </Link>
          {showAdminBootstrap && (
            <Link href={`/${locale}/admin-bootstrap?email=${encodeURIComponent(email)}`} className="text-xs text-amber-500 hover:underline mt-2">
              {pickText(lang, "初始化管理员 TOTP", "Bootstrap admin TOTP")}
            </Link>
          )}
        </div>
      </Card>
    </div>
  );
}
