import Link from "next/link";
import { cookies } from "next/headers";
import { KeyRound } from "lucide-react";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { firstValue } from "@/lib/auth";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type PasswordResetPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    code?: string | string[];
    email?: string | string[];
    error?: string | string[];
    notice?: string | string[];
    step?: string | string[];
  }>;
};

export default async function PasswordResetPage({ params, searchParams }: PasswordResetPageProps) {
  const { locale } = await params;
  const [searchParamsValue, cookieStore] = await Promise.all([searchParams, cookies()]);
  const resolved = searchParamsValue ?? {};
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const email = firstValue(resolved.email) ?? "";
  const code = firstValue(resolved.code) ?? "";
  const error = firstValue(resolved.error);
  const notice = firstValue(resolved.notice);
  const step = firstValue(resolved.step) === "confirm" ? "confirm" : "request";

  return (
    <div className="w-full max-w-[420px] space-y-6">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-bold tracking-tight text-foreground">
          {step === "confirm" ? pickText(lang, "设置新密码", "Reset your password") : pickText(lang, "密码重置", "Password Reset")}
        </h1>
        <p className="text-sm text-muted-foreground">
          {step === "confirm" 
            ? pickText(lang, "输入邮箱收到的验证码，完成密码重置。", "Complete the reset with the code sent to your email inbox.") 
            : pickText(lang, "先申请验证码，再去邮箱查看。", "Request a password reset code first, then check your email for the code.")}
        </p>
      </div>

      {error ? (
        <StatusBanner description={error} title={pickText(lang, "密码重置失败", "Password reset failed")} tone="danger" />
      ) : (
        <StatusBanner
          description={step === "confirm" && notice === "reset-code-issued"
            ? pickText(lang, "请先去邮箱查收重置验证码，再输入新密码完成修改。", "Check your email for the issued reset code, then enter it with your new password.")
            : pickText(lang, "先申请重置验证码，再去邮箱确认并设置新密码。", "Request a reset code, then check your email and confirm the new password.")}
          title={step === "confirm" && notice === "reset-code-issued"
            ? pickText(lang, "重置验证码已发送", "Reset code issued")
            : step === "confirm"
              ? pickText(lang, "确认重置密码", "Confirm Password Reset")
              : pickText(lang, "密码重置", "Password Reset")}
          tone="info"
        />
      )}

      <Card className="bg-card border-border shadow-xl">
        <CardBody className="p-6">
          <FormStack action={`/api/auth/password-reset?locale=${locale}`} method="post" className="space-y-5">
            <input name="intent" type="hidden" value={step} />
            
            <Field label={pickText(lang, "邮箱", "Email")}>
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

            {step === "confirm" && (
              <>
                <Field hint={pickText(lang, "请输入这次密码重置邮件里的验证码。", "Enter the reset code delivered to your email.")} label={pickText(lang, "重置验证码", "Reset code")}>
                  <Input 
                    defaultValue={code} 
                    inputMode="numeric" 
                    name="code" 
                    required 
                    pattern="[0-9]{6}" 
                    className="bg-input border-border h-10 font-mono text-center tracking-widest text-lg"
                    placeholder="000000"
                  />
                </Field>
                <Field label={pickText(lang, "新密码", "New password")}>
                  <Input 
                    autoComplete="new-password" 
                    name="password" 
                    required 
                    type="password" 
                    className="bg-input border-border h-10 text-sm"
                    placeholder="••••••••"
                  />
                </Field>
              </>
            )}

            <Button type="submit" tone="primary" className="w-full h-11 text-sm font-bold shadow-lg shadow-primary/20">
              <KeyRound className="w-4 h-4 mr-2" />
              {step === "confirm" ? pickText(lang, "重置密码", "Reset password") : pickText(lang, "发送重置验证码", "Send reset code")}
            </Button>
          </FormStack>
        </CardBody>
        <div className="border-t border-border/60 bg-secondary/30 p-4 text-center flex flex-col gap-2">
          <Link href={`/${locale}/login`} className="text-xs text-muted-foreground hover:text-foreground transition-colors">
            {pickText(lang, "返回登录", "Back to login")}
          </Link>
          <Link href={`/${locale}/register`} className="text-xs text-primary hover:underline font-semibold">
            {pickText(lang, "还没有账号？去注册", "Need a new account? Register")}
          </Link>
        </div>
      </Card>
    </div>
  );
}
