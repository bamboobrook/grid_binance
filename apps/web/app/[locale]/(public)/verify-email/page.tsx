import Link from "next/link";
import { cookies } from "next/headers";
import { MailCheck } from "lucide-react";

import { Card, CardBody } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { firstValue, safeRedirectTarget } from "@/lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type VerifyEmailPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    code?: string | string[];
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
    notice?: string | string[];
  }>;
};

export default async function VerifyEmailPage({ params, searchParams }: VerifyEmailPageProps) {
  const { locale } = await params;
  const [searchParamsValue, cookieStore] = await Promise.all([searchParams, cookies()]);
  const resolved = searchParamsValue ?? {};
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const email = firstValue(resolved.email) ?? "";
  const code = firstValue(resolved.code) ?? "";
  const error = firstValue(resolved.error);
  const next = safeRedirectTarget(firstValue(resolved.next), "/app/dashboard");
  const notice = firstValue(resolved.notice);

  return (
    <div className="w-full max-w-[420px] space-y-6">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-bold tracking-tight text-foreground">
          {pickText(lang, "验证邮箱", "Verify Email")}
        </h1>
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "请输入注册邮箱收到的验证码，完成首次验证。", "Confirm the verification code delivered to the email address used during registration.")}
        </p>
      </div>

      {error ? (
        <StatusBanner description={error} title={pickText(lang, "邮箱验证失败", "Email verification failed")} tone="danger" />
      ) : (
        <StatusBanner
          description={notice === "registration-created"
            ? pickText(lang, "首次登录前，请先去邮箱查看验证码。", "Check your email for the issued verification code before your first login.")
            : pickText(lang, "必须先完成邮箱验证，系统已把验证码发到你的邮箱。", "Verification must complete before login is allowed. The code is sent to your email address.")}
          title={pickText(lang, "验证邮箱", "Verify Email")}
          tone="info"
        />
      )}

      <Card className="bg-card border-border shadow-xl">
        <CardBody className="p-6">
          <FormStack action={`/api/auth/verify-email?locale=${locale}`} method="post" className="space-y-5">
            <input name="next" type="hidden" value={next} />
            
            <Field label="Email">
              <Input 
                autoComplete="email" 
                defaultValue={email} 
                name="email" 
                required 
                type="email" 
                className="bg-input border-border h-10 text-sm"
              />
            </Field>

            <Field hint={pickText(lang, "请输入邮箱里收到的 6 位验证码。", "Enter the verification code delivered to your email inbox.")} label={pickText(lang, "验证码", "Verification code")}>
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

            <Button type="submit" tone="primary" className="w-full h-11 text-sm font-bold shadow-lg shadow-primary/20">
              <MailCheck className="w-4 h-4 mr-2" />
              {pickText(lang, "验证邮箱", "Verify email")}
            </Button>
          </FormStack>
        </CardBody>
        <div className="border-t border-border/60 bg-secondary/30 p-4 text-center flex flex-col gap-2">
          <Link href={`/${locale}/login`} className="text-xs text-muted-foreground hover:text-foreground transition-colors">
            {pickText(lang, "返回登录", "Back to login")}
          </Link>
          <Link href={`/${locale}/register`} className="text-xs text-primary hover:underline font-semibold">
            {pickText(lang, "如果邮箱填错了，请重新注册", "Need to change email? Register again")}
          </Link>
        </div>
      </Card>
    </div>
  );
}
