import Link from "next/link";
import { cookies } from "next/headers";
import { CheckCircle2, ShieldAlert, UserPlus } from "lucide-react";

import { Card, CardBody } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { getPublicAuthSnapshot } from "@/lib/api/server";
import { firstValue, safeRedirectTarget } from "@/lib/auth";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

type RegisterPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
  }>;
};

export default async function RegisterPage({ params, searchParams }: RegisterPageProps) {
  const { locale } = await params;
  const [snapshot, cookieStore] = await Promise.all([getPublicAuthSnapshot("register", locale), cookies()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const searchParamsValue = (await searchParams) ?? {};
  const email = firstValue(searchParamsValue.email) ?? "";
  const error = firstValue(searchParamsValue.error);
  const next = safeRedirectTarget(firstValue(searchParamsValue.next), "/" + locale + "/app/dashboard");
  const setupSteps = [
    pickText(lang, "先创建账号，进入控制台。", "Create your account and enter the console."),
    pickText(lang, "绑定币安 API，确认没有提现权限。", "Connect Binance API and confirm withdrawal access is off."),
    pickText(lang, "选择模板，小额启动第一个机器人。", "Choose a template and start the first bot with small capital."),
  ];

  return (
    <div className="grid min-h-[85vh] w-full items-center gap-8 bg-background px-4 py-10 text-foreground sm:px-6 lg:grid-cols-[minmax(0,1fr)_28rem] lg:px-10">
      <section className="mx-auto w-full max-w-2xl lg:mx-0">
        <div className="inline-flex items-center gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs font-semibold text-amber-700 dark:text-amber-300">
          <ShieldAlert className="h-4 w-4" />
          {pickText(lang, "先小额试跑，再扩大资金", "Start small before increasing capital")}
        </div>
        <h1 className="mt-5 text-3xl font-black tracking-tight sm:text-4xl">
          {pickText(lang, "创建账号后，系统会带你一步步完成机器人设置。", "After registration, the console guides you through bot setup step by step.")}
        </h1>
        <p className="mt-3 max-w-xl text-sm leading-6 text-muted-foreground sm:text-base">
          {pickText(
            lang,
            "你不需要一开始就理解所有参数。先完成安全连接，再用模板创建普通网格；马丁组合和高级设置可以之后再打开。",
            "You do not need every parameter on day one. Connect safely, create a simple grid from a template, and open DCA or advanced settings later.",
          )}
        </p>
        <div className="mt-6 space-y-3">
          {setupSteps.map((step, index) => (
            <div className="grid grid-cols-[2rem_1fr] gap-3 rounded-md border border-border bg-card p-3" key={step}>
              <span className="flex h-8 w-8 items-center justify-center rounded-md bg-primary/10 text-sm font-bold text-primary">{index + 1}</span>
              <span className="self-center text-sm font-medium">{step}</span>
            </div>
          ))}
        </div>
      </section>

      <div className="mx-auto w-full max-w-md space-y-6">
        <div>
          <h2 className="text-2xl font-extrabold tracking-tight text-foreground">{snapshot.title}</h2>
          <p className="mt-2 text-sm text-muted-foreground">{snapshot.description}</p>
        </div>

        {error ? (
          <StatusBanner description={error} lang={lang} title={pickText(lang, "注册失败", "Registration failed")} tone="error" />
        ) : snapshot.notice.description ? (
          <StatusBanner description={snapshot.notice.description} lang={lang} title={snapshot.notice.title} tone={snapshot.notice.tone as any} />
        ) : null}

        <Card className="overflow-hidden rounded-md border-border bg-card shadow-sm">
          <CardBody className="p-8">
            <FormStack action={"/api/auth/register?locale=" + locale} method="post" className="space-y-6">
              <input name="next" type="hidden" value={next} />

              <Field label={pickText(lang, "邮箱", "Email")} hint={pickText(lang, "注册后可直接登录", "Sign in right after registration")}>
                <Input
                  autoComplete="email"
                  defaultValue={email}
                  name="email"
                  required
                  type="email"
                  className="h-12 w-full rounded-md border-border bg-background px-4 text-foreground focus:border-primary focus:ring-primary"
                  placeholder={pickText(lang, "name@example.com", "name@example.com")}
                />
              </Field>

              <Field label={pickText(lang, "密码", "Password")}>
                <Input
                  autoComplete="new-password"
                  name="password"
                  required
                  type="password"
                  className="h-12 w-full rounded-md border-border bg-background px-4 text-foreground focus:border-primary focus:ring-primary"
                  placeholder="••••••••"
                />
              </Field>

              <Button type="submit" tone="primary" className="h-12 w-full rounded-md text-base font-bold shadow-sm transition-all hover:bg-primary/90">
                <UserPlus className="w-5 h-5 mr-2" />
                {snapshot.submitLabel}
              </Button>
            </FormStack>
          </CardBody>
          <div className="border-t border-border bg-background p-5 text-center flex flex-col gap-3">
            <Link href={"/" + locale + "/login"} className="text-sm font-semibold text-primary transition-colors hover:text-primary hover:underline">
              {snapshot.alternateLabel}
            </Link>
          </div>
        </Card>

        <div className="text-center mt-6">
          <p className="mx-auto flex max-w-xs items-start justify-center gap-2 text-xs leading-relaxed text-muted-foreground">
            <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-500" />
            <span>
            {pickText(lang, "注册即表示您同意我们的服务条款和隐私政策。一账户仅限绑定一个交易所 API。", "By registering, you agree to our Terms of Service and Privacy Policy. One exchange API per account.")}
            </span>
          </p>
        </div>
      </div>
    </div>
  );
}
