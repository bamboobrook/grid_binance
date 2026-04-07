import Link from "next/link";
import { cookies } from "next/headers";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../components/ui/card";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../components/ui/form";
import { StatusBanner } from "../../../components/ui/status-banner";
import { Tabs } from "../../../components/ui/tabs";
import { firstValue, safeRedirectTarget } from "../../../lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "../../../lib/ui/preferences";

type VerifyEmailPageProps = {
  searchParams?: Promise<{
    code?: string | string[];
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
    notice?: string | string[];
  }>;
};

export default async function VerifyEmailPage({ searchParams }: VerifyEmailPageProps) {
  const [params, cookieStore] = await Promise.all([searchParams, cookies()]);
  const resolved = (await params) ?? {};
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const email = firstValue(resolved.email) ?? "";
  const code = firstValue(resolved.code) ?? "";
  const error = firstValue(resolved.error);
  const next = safeRedirectTarget(firstValue(resolved.next), "/app/dashboard");
  const notice = firstValue(resolved.notice);

  return (
    <>
      <Tabs
        activeHref="/register"
        items={[
          { href: "/login", label: pickText(lang, "登录", "Login") },
          { href: "/register", label: pickText(lang, "注册", "Register") },
        ]}
        label={pickText(lang, "认证页面", "Authentication pages")}
      />
      {error ? (
        <StatusBanner description={error} title={pickText(lang, "邮箱验证失败", "Email verification failed")} tone="danger" />
      ) : (
        <StatusBanner
          description={notice === "registration-created"
            ? pickText(lang, "首次登录前，请先去邮箱查看验证码。", "Check your email for the issued verification code before your first login.")
            : pickText(lang, "必须先完成邮箱验证，系统已把验证码发到你的邮箱。", "Verification must complete before login is allowed. The code is sent to your email address.")}
          title={pickText(lang, "验证邮箱", "Verify Email")}
          tone="warning"
        />
      )}
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>{pickText(lang, "验证邮箱", "Verify Email")}</CardTitle>
            <CardDescription>{pickText(lang, "请输入注册邮箱收到的验证码，完成首次验证。", "Confirm the verification code delivered to the email address used during registration.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/verify-email" method="post">
              <input name="next" type="hidden" value={next} />
              <Field label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field hint={pickText(lang, "请输入邮箱里收到的 6 位验证码。", "Enter the verification code delivered to your email inbox before the first login.")} label={pickText(lang, "验证码", "Verification code")}>
                <Input defaultValue={code} inputMode="numeric" name="code" required pattern="[0-9]{6}" />
              </Field>
              <ButtonRow>
                <Button type="submit">{pickText(lang, "验证邮箱", "Verify email")}</Button>
                <Link className="button button--ghost" href="/login">
                  {pickText(lang, "返回登录", "Back to login")}
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href="/register">{pickText(lang, "如果邮箱填错了，请重新注册", "Need to change email? Register again")}</Link>
          </CardFooter>
        </Card>
      </div>
    </>
  );
}
