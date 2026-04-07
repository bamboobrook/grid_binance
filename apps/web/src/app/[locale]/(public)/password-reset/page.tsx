import Link from "next/link";
import { cookies } from "next/headers";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { Tabs } from "../../../../components/ui/tabs";
import { firstValue } from "../../../../lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "../../../../lib/ui/preferences";

type PasswordResetPageProps = {
  searchParams?: Promise<{
    code?: string | string[];
    email?: string | string[];
    error?: string | string[];
    notice?: string | string[];
    step?: string | string[];
  }>;
};

export default async function PasswordResetPage({ searchParams }: PasswordResetPageProps) {
  const [params, cookieStore] = await Promise.all([searchParams, cookies()]);
  const resolved = (await params) ?? {};
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const email = firstValue(resolved.email) ?? "";
  const code = firstValue(resolved.code) ?? "";
  const error = firstValue(resolved.error);
  const notice = firstValue(resolved.notice);
  const step = firstValue(resolved.step) === "confirm" ? "confirm" : "request";

  return (
    <>
      <Tabs
        activeHref="/login"
        items={[
          { href: "/login", label: pickText(lang, "登录", "Login") },
          { href: "/register", label: pickText(lang, "注册", "Register") },
        ]}
        label={pickText(lang, "认证页面", "Authentication pages")}
      />
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
          tone="warning"
        />
      )}
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>{step === "confirm" ? pickText(lang, "设置新密码", "Reset your password") : pickText(lang, "申请重置验证码", "Request reset code")}</CardTitle>
            <CardDescription>{step === "confirm" ? pickText(lang, "输入邮箱收到的验证码，完成密码重置。", "Complete the reset with the code sent to your email inbox.") : pickText(lang, "先申请验证码，再去邮箱查看。", "Request a password reset code first, then check your email for the code.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/password-reset" method="post">
              <input name="intent" type="hidden" value={step} />
              <Field label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              {step === "confirm" ? (
                <>
                  <Field hint={pickText(lang, "请输入这次密码重置邮件里的验证码。", "Enter the reset code delivered to your email for this password reset request.")} label={pickText(lang, "重置验证码", "Reset code")}>
                    <Input defaultValue={code} inputMode="numeric" name="code" required pattern="[0-9]{6}" />
                  </Field>
                  <Field label={pickText(lang, "新密码", "New password")}>
                    <Input autoComplete="new-password" name="password" required type="password" />
                  </Field>
                </>
              ) : null}
              <ButtonRow>
                <Button type="submit">{step === "confirm" ? pickText(lang, "重置密码", "Reset password") : pickText(lang, "发送重置验证码", "Send reset code")}</Button>
                <Link className="button button--ghost" href="/login">
                  {pickText(lang, "返回登录", "Back to login")}
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href="/register">{pickText(lang, "还没有账号？去注册", "Need a new account? Register")}</Link>
          </CardFooter>
        </Card>
      </div>
    </>
  );
}
