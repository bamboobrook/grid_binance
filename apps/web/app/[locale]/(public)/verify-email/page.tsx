import Link from "next/link";
import { cookies } from "next/headers";
import { MailCheck } from "lucide-react";

import { Card, CardBody } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { firstValue, safeRedirectTarget } from "@/lib/auth";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

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
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const email = firstValue(resolved.email) ?? "";
  const code = firstValue(resolved.code) ?? "";
  const error = firstValue(resolved.error);
  const next = safeRedirectTarget(firstValue(resolved.next), "/app/dashboard");
  const notice = firstValue(resolved.notice);
  const isLegacyNotice = notice === "legacy-only";
  const isRegistrationNotice = notice === "registration-created";

  return (
    <div className="w-full max-w-[420px] space-y-6">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-bold tracking-tight text-foreground">
          {pickText(lang, "邮箱确认", "Email confirmation")}
        </h1>
        <p className="text-sm text-muted-foreground">
          {pickText(
            lang,
            "新账号现在已可直接登录；这个页面只保留给旧验证码或人工补验证场景。",
            "New accounts can sign in now. This page remains only for legacy verification codes or manual backfill flows.",
          )}
        </p>
      </div>

      {error ? (
        <StatusBanner
                tone="info"
                lang={lang}
          description={error}
          title={pickText(lang, "旧验证码提交失败", "Legacy verification failed")}
        />
      ) : (
        <StatusBanner
                tone="info"
                lang={lang}
          description={isRegistrationNotice
            ? pickText(
                lang,
                "注册已完成，账号已可直接登录；如果你是从旧邮件链接或人工补验证流程进入，仍可在下方提交旧验证码。",
                "Registration is complete and your account can sign in now. If you opened this page from a legacy email link or a manual backfill flow, you can still submit the old code below.",
              )
            : isLegacyNotice
              ? pickText(
                  lang,
                  "这是旧验证码兼容入口：只有历史邮件链接或人工补验证场景才需要继续停留在这里。",
                  "This is the legacy compatibility entrypoint: stay here only for historical email links or manual backfill verification.",
                )
            : pickText(
                lang,
                "大多数账号不再依赖邮箱验证登录；这里只保留旧验证码兼容入口。",
                "Most accounts no longer need email verification before sign-in. This form stays only as a legacy compatibility entrypoint.",
              )}
          title={pickText(lang, "账号已可登录", "Account ready")}
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

            <Field
              hint={pickText(
                lang,
                "如果你手上还有旧的 6 位验证码，可在这里提交；否则直接返回登录即可。",
                "If you still have a legacy 6-digit verification code, submit it here. Otherwise, go straight to login.",
              )}
              label={pickText(lang, "旧验证码", "Legacy verification code")}
            >
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
              {pickText(lang, "提交旧验证码", "Submit legacy code")}
            </Button>
          </FormStack>
        </CardBody>
        <div className="border-t border-border/60 bg-secondary/30 p-4 text-center flex flex-col gap-2">
          <Link href={`/${locale}/login`} className="text-xs text-muted-foreground hover:text-foreground transition-colors">
            {pickText(lang, "直接去登录", "Go to login")}
          </Link>
          <Link href={`/${locale}/register`} className="text-xs text-primary hover:underline font-semibold">
            {pickText(lang, "如果邮箱填错了，请重新注册", "Need to change email? Register again")}
          </Link>
        </div>
      </Card>
    </div>
  );
}
