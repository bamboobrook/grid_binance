import Link from "next/link";
import { cookies } from "next/headers";
import { UserPlus } from "lucide-react";

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

  return (
    <div className="flex flex-col items-center justify-center min-h-[85vh] py-12 px-4 sm:px-6 lg:px-8 bg-[#0a0e17] text-slate-200 w-full">
      <div className="w-full max-w-md space-y-8">
        <div className="text-center">
          <h1 className="text-3xl font-extrabold tracking-tight text-white">{snapshot.title}</h1>
          <p className="mt-2 text-sm text-slate-400">{snapshot.description}</p>
        </div>

        {error ? (
          <StatusBanner description={error} title={pickText(lang, "注册失败", "Registration failed")} tone="danger" />
        ) : snapshot.notice.description ? (
          <StatusBanner description={snapshot.notice.description} title={snapshot.notice.title} tone={snapshot.notice.tone as any} />
        ) : null}

        <Card className="bg-[#111827] border-slate-800 shadow-2xl rounded-2xl overflow-hidden">
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
                  className="bg-[#1f2937] border-slate-700 text-white focus:ring-primary focus:border-primary h-12 rounded-lg px-4 w-full"
                  placeholder={pickText(lang, "name@example.com", "name@example.com")}
                />
              </Field>

              <Field label={pickText(lang, "密码", "Password")}>
                <Input
                  autoComplete="new-password"
                  name="password"
                  required
                  type="password"
                  className="bg-[#1f2937] border-slate-700 text-white focus:ring-primary focus:border-primary h-12 rounded-lg px-4 w-full"
                  placeholder="••••••••"
                />
              </Field>

              <Button type="submit" tone="primary" className="w-full h-12 text-base font-bold shadow-lg shadow-primary/30 rounded-lg hover:bg-primary/90 transition-all">
                <UserPlus className="w-5 h-5 mr-2" />
                {snapshot.submitLabel}
              </Button>
            </FormStack>
          </CardBody>
          <div className="border-t border-slate-800 bg-[#0f141f] p-5 text-center flex flex-col gap-3">
            <Link href={"/" + locale + "/login"} className="text-sm text-primary hover:text-primary-foreground font-semibold hover:underline transition-colors">
              {snapshot.alternateLabel}
            </Link>
          </div>
        </Card>

        <div className="text-center mt-6">
          <p className="text-xs text-slate-500 max-w-xs mx-auto leading-relaxed">
            {pickText(lang, "注册即表示您同意我们的服务条款和隐私政策。一账户仅限绑定一个交易所 API。", "By registering, you agree to our Terms of Service and Privacy Policy. One exchange API per account.")}
          </p>
        </div>
      </div>
    </div>
  );
}

