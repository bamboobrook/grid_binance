import Link from "next/link";
import { cookies } from "next/headers";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "../../../../components/ui/card";
import { Chip } from "../../../../components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Input } from "../../../../components/ui/form";
import { StatusBanner } from "../../../../components/ui/status-banner";
import { Tabs } from "../../../../components/ui/tabs";
import { getPublicAuthSnapshot } from "../../../../lib/api/server";
import { firstValue, safeRedirectTarget } from "../../../../lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "../../../../lib/ui/preferences";

type LoginPageProps = {
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

export default async function LoginPage({ searchParams }: LoginPageProps) {
  const [snapshot, cookieStore] = await Promise.all([getPublicAuthSnapshot("login"), cookies()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const next = safeRedirectTarget(firstValue(params.next), "/app/dashboard");
  const notice = noticeCopy(lang, firstValue(params.notice) ?? firstValue(params.security));
  const showTotp = firstValue(params.totp) === "1" || Boolean(error && /totp/i.test(error));
  const showAdminBootstrap = firstValue(params.adminBootstrap) === "1" || Boolean(error && /admin totp setup required/i.test(error ?? ""));
  const reminders = [
    pickText(lang, "会员到期前，网页和 Telegram 都会提前提醒。", "Membership expiry reminders appear in web and Telegram before grace ends."),
    pickText(lang, "币安 API 不要开启提现权限。", "Do not enable withdrawal permission on Binance API keys."),
    pickText(lang, "TOTP 可以稍后在安全中心开启。", "TOTP can be enabled later from the security center."),
  ];

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
        <StatusBanner description={error} title={pickText(lang, "登录失败", "Login failed")} tone="danger" />
      ) : notice ? (
        <StatusBanner description={notice.description} title={notice.title} tone="success" />
      ) : (
        <StatusBanner description={snapshot.notice.description} title={snapshot.notice.title} tone={snapshot.notice.tone} />
      )}
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>{snapshot.title}</CardTitle>
            <CardDescription>{snapshot.description}</CardDescription>
          </CardHeader>
          <CardBody>
            <FormStack action="/api/auth/login" method="post">
              <input name="next" type="hidden" value={next} />
              <Field hint={pickText(lang, "请输入已经完成验证、并已绑定会员与交易所设置的邮箱。", "Use the verified email tied to your membership and exchange setup.")} label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field hint={pickText(lang, "密码登录是第一步，如有需要，系统会继续要求输入 TOTP。", "Password login remains the first step before optional TOTP challenges.")} label={pickText(lang, "密码", "Password")}>
                <Input autoComplete="current-password" name="password" required type="password" />
              </Field>
              {showTotp ? (
                <Field hint={pickText(lang, "请输入验证器当前显示的 6 位验证码。", "Enter the current 6-digit code from your authenticator app.")} label="TOTP">
                  <Input autoComplete="one-time-code" inputMode="numeric" name="totpCode" pattern="[0-9]{6}" />
                </Field>
              ) : null}
              <div className="chip-row">
                {snapshot.checklist.map((item) => (
                  <Chip key={item} tone="info">
                    {item}
                  </Chip>
                ))}
              </div>
              <ButtonRow>
                <Button type="submit">{snapshot.submitLabel}</Button>
                <Link className="button button--ghost" href="/password-reset">
                  {pickText(lang, "重置密码", "Reset password")}
                </Link>
                <Link className="button button--ghost" href="/help/expiry-reminder">
                  {pickText(lang, "查看到期提醒", "Review expiry reminders")}
                </Link>
                {showAdminBootstrap ? (
                  <Link className="button button--ghost" href={`/admin-bootstrap?email=${encodeURIComponent(email)}`}>
                    {pickText(lang, "初始化管理员 TOTP", "Bootstrap admin TOTP")}
                  </Link>
                ) : null}
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href={snapshot.alternateHref}>{snapshot.alternateLabel}</Link>
          </CardFooter>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "登录前请确认", "Before you sign in")}</CardTitle>
            <CardDescription>{pickText(lang, "公共认证页也会保留关键商用提醒。", "Commercial guardrails stay visible on the public auth pages too.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {reminders.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
