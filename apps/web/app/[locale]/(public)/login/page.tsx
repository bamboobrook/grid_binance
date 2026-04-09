import Link from "next/link";
import { cookies } from "next/headers";
import { LogIn } from "lucide-react";

import { Card, CardBody } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { getPublicAuthSnapshot } from "@/lib/api/server";
import { firstValue, safeRedirectTarget } from "@/lib/auth";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

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
    case "registration-created":
      return {
        title: pickText(lang, "账号已创建", "Account created"),
        description: pickText(lang, "注册已完成，现在可以直接登录；如果你已经启用 TOTP，请一并填写当前验证码。", "Registration is complete. You can sign in now. If TOTP is already enabled, enter the current code as well."),
      };
    case "email-verified":
      return {
        title: pickText(lang, "邮箱验证完成", "Email verified"),
        description: pickText(lang, "邮箱已经验证，现在可以登录；如果你已经启用 TOTP，请一并填写当前验证码。", "Your email is verified. You can sign in now. If TOTP is already enabled, enter the current code as well."),
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
  const [snapshot, cookieStore] = await Promise.all([getPublicAuthSnapshot("login", locale), cookies()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const searchParamsValue = (await searchParams) ?? {};
  const email = firstValue(searchParamsValue.email) ?? "";
  const error = firstValue(searchParamsValue.error);
  const requestedNext = firstValue(searchParamsValue.next);
  const next = requestedNext ? safeRedirectTarget(requestedNext, "/" + locale + "/app/dashboard") : "";
  const notice = noticeCopy(lang, firstValue(searchParamsValue.notice) ?? firstValue(searchParamsValue.security));
  const showAdminBootstrap = firstValue(searchParamsValue.adminBootstrap) === "1" || Boolean(error && /admin totp setup required/i.test(error ?? ""));

  return (
    <div className="flex flex-col items-center justify-center min-h-[85vh] py-12 px-4 sm:px-6 lg:px-8 bg-[#0a0e17] text-slate-200 w-full">
      <div className="w-full max-w-md space-y-8">
        <div className="text-center">
          <h1 className="text-3xl font-extrabold tracking-tight text-white">{snapshot.title}</h1>
          <p className="mt-2 text-sm text-slate-400">{snapshot.description}</p>
        </div>

        {error ? (
          <StatusBanner description={error} title={pickText(lang, "登录失败", "Login failed")} tone="danger" />
        ) : notice ? (
          <StatusBanner description={notice.description} title={notice.title} tone="success" />
        ) : snapshot.notice.description ? (
          <StatusBanner description={snapshot.notice.description} title={snapshot.notice.title} tone={snapshot.notice.tone as any} />
        ) : null}

        <Card className="bg-[#111827] border-slate-800 shadow-2xl rounded-2xl overflow-hidden">
          <CardBody className="p-8">
            <FormStack action={"/api/auth/login?locale=" + locale} method="post" className="space-y-6">
              <input name="next" type="hidden" value={next} />

              <Field label={pickText(lang, "邮箱", "Email")}>
                <Input
                  autoComplete="email"
                  defaultValue={email}
                  name="email"
                  required
                  type="email"
                  className="bg-[#1f2937] border-slate-700 text-white focus:ring-primary focus:border-primary h-12 rounded-lg px-4 w-full"
                  placeholder={pickText(lang, "name@example.com", "name@example.com")}
                />
              </Field>

              <Field label={pickText(lang, "密码", "Password")}>
                <Input
                  autoComplete="current-password"
                  name="password"
                  required
                  type="password"
                  className="bg-[#1f2937] border-slate-700 text-white focus:ring-primary focus:border-primary h-12 rounded-lg px-4 w-full"
                  placeholder="••••••••"
                />
              </Field>

              <Field
                label={pickText(lang, "TOTP 验证码", "TOTP code")}
                hint={pickText(lang, "如果尚未启用 TOTP，可先留空；如果已启用，则必须填写当前 6 位验证码。", "Leave it blank if TOTP is not enabled yet. If it is already enabled, enter the current 6-digit code.")}
              >
                <Input
                  autoComplete="one-time-code"
                  inputMode="numeric"
                  name="totpCode"
                  pattern="[0-9]{6}"
                  className="bg-[#1f2937] border-slate-700 text-white focus:ring-primary focus:border-primary h-12 rounded-lg px-4 font-mono text-center tracking-[0.5em] text-lg w-full"
                  placeholder="000000"
                />
              </Field>

              <Button type="submit" tone="primary" className="w-full h-12 text-base font-bold shadow-lg shadow-primary/30 rounded-lg hover:bg-primary/90 transition-all">
                <LogIn className="w-5 h-5 mr-2" />
                {snapshot.submitLabel}
              </Button>
            </FormStack>
          </CardBody>
          <div className="border-t border-slate-800 bg-[#0f141f] p-5 text-center flex flex-col gap-3">
            <Link href={"/" + locale + "/password-reset"} className="text-sm text-slate-400 hover:text-white transition-colors">
              {pickText(lang, "忘记密码？重置密码", "Forgot password? Reset here")}
            </Link>
            <Link href={"/" + locale + "/register"} className="text-sm text-primary hover:text-primary-foreground font-semibold hover:underline transition-colors">
              {snapshot.alternateLabel}
            </Link>
            {showAdminBootstrap ? (
              <Link href={"/" + locale + "/admin-bootstrap?email=" + encodeURIComponent(email)} className="text-xs text-amber-500 hover:text-amber-400 hover:underline mt-2">
                {pickText(lang, "初始化管理员 TOTP", "Bootstrap admin TOTP")}
              </Link>
            ) : null}
          </div>
        </Card>
      </div>
    </div>
  );
}

