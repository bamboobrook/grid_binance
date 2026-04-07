import Link from "next/link";
import { cookies } from "next/headers";
import { UserPlus } from "lucide-react";

import { Card, CardBody } from "@/components/ui/card";
import { Button, Field, FormStack, Input } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { getPublicAuthSnapshot } from "@/lib/api/server";
import { firstValue, safeRedirectTarget } from "@/lib/auth";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE } from "@/lib/ui/preferences";

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
  const [snapshot, cookieStore] = await Promise.all([getPublicAuthSnapshot("register"), cookies()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const searchParamsValue = (await searchParams) ?? {};
  const email = firstValue(searchParamsValue.email) ?? "";
  const error = firstValue(searchParamsValue.error);
  const next = safeRedirectTarget(firstValue(searchParamsValue.next), "/app/dashboard");

  return (
    <div className="w-full max-w-[420px] space-y-6">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-bold tracking-tight text-foreground">{snapshot.title}</h1>
        <p className="text-sm text-muted-foreground">{snapshot.description}</p>
      </div>

      {error ? (
        <StatusBanner description={error} title={pickText(lang, "注册失败", "Registration failed")} tone="danger" />
      ) : snapshot.notice.description ? (
        <StatusBanner description={snapshot.notice.description} title={snapshot.notice.title} tone={snapshot.notice.tone as any} />
      ) : null}

      <Card className="bg-card border-border shadow-xl">
        <CardBody className="p-6">
          <FormStack action={`/api/auth/register?locale=${locale}`} method="post" className="space-y-5">
            <input name="next" type="hidden" value={next} />
            
            <Field label="Email" hint={pickText(lang, "需要进行验证", "Verification required")}>
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

            <Field label={pickText(lang, "密码", "Password")}>
              <Input 
                autoComplete="new-password" 
                name="password" 
                required 
                type="password" 
                className="bg-input border-border h-10 text-sm"
                placeholder="••••••••"
              />
            </Field>

            <Button type="submit" tone="primary" className="w-full h-11 text-sm font-bold shadow-lg shadow-primary/20">
              <UserPlus className="w-4 h-4 mr-2" />
              {snapshot.submitLabel}
            </Button>
          </FormStack>
        </CardBody>
        <div className="border-t border-border/60 bg-secondary/30 p-4 text-center flex flex-col gap-2">
          <Link href={`/${locale}/login`} className="text-xs text-primary hover:underline font-semibold">
            {snapshot.alternateLabel}
          </Link>
        </div>
      </Card>

      <div className="text-center">
        <p className="text-[11px] text-muted-foreground max-w-xs mx-auto leading-relaxed">
          {pickText(lang, "注册即表示您同意我们的服务条款和隐私政策。一账户仅限绑定一个交易所 API。", "By registering, you agree to our Terms of Service and Privacy Policy. One exchange API per account.")}
        </p>
      </div>
    </div>
  );
}
