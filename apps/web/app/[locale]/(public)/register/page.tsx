import Link from "next/link";
import { cookies } from "next/headers";

import { Card, CardBody, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { Button, ButtonRow, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { Tabs } from "@/components/ui/tabs";
import { getPublicAuthSnapshot } from "@/lib/api/server";
import { firstValue, safeRedirectTarget } from "@/lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type RegisterPageProps = {
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
  }>;
};

export default async function RegisterPage({ searchParams }: RegisterPageProps) {
  const [snapshot, cookieStore] = await Promise.all([getPublicAuthSnapshot("register"), cookies()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const next = safeRedirectTarget(firstValue(params.next), "/app/dashboard");
  const onboardingNotes = [
    pickText(lang, "必须先完成邮箱验证，才能正常登录。", "Email verification is required before normal sign-in is allowed."),
    pickText(lang, "每个用户只能绑定一个币安账号，保持所有权关系清晰。", "One Binance account per user keeps exchange ownership explicit."),
    pickText(lang, "没有会员权限时，不允许启动任何策略。", "Membership is required before any strategy can start."),
  ];

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
        <StatusBanner description={error} title={pickText(lang, "注册失败", "Registration failed")} tone="danger" />
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
            <FormStack action="/api/auth/register" method="post">
              <input name="next" type="hidden" value={next} />
              <Field hint={pickText(lang, "注册后会先发送验证码，验证完成后才能登录。", "A verification code will be issued before login is allowed.")} label="Email">
                <Input autoComplete="email" defaultValue={email} name="email" required type="email" />
              </Field>
              <Field hint={pickText(lang, "请先设置独立密码，之后可到安全中心启用 TOTP。", "Use a unique password before enabling TOTP in the security center.")} label={pickText(lang, "密码", "Password")}>
                <Input autoComplete="new-password" name="password" required type="password" />
              </Field>
              <div className="chip-row">
                {snapshot.checklist.map((item) => (
                  <Chip key={item} tone="warning">
                    {item}
                  </Chip>
                ))}
              </div>
              <ButtonRow>
                <Button type="submit">{snapshot.submitLabel}</Button>
                <Link className="button button--ghost" href="/help/expiry-reminder">
                  {pickText(lang, "查看计费说明", "Billing help")}
                </Link>
              </ButtonRow>
            </FormStack>
          </CardBody>
          <CardFooter>
            <Link href={snapshot.alternateHref}>{snapshot.alternateLabel}</Link>
          </CardFooter>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "开户流程", "Account onboarding")}</CardTitle>
            <CardDescription>{pickText(lang, "注册后必须先完成验证，才会进入正式登录。", "Registration now moves through explicit verification before login.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {onboardingNotes.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
