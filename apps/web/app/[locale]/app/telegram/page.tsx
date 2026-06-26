import { redirect } from "next/navigation";

type TelegramPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
};

export default async function TelegramPage({ params, searchParams }: TelegramPageProps) {
  const { locale } = await params;
  const query = await searchParams;
  const target = new URLSearchParams();

  for (const [key, value] of Object.entries(query ?? {})) {
    if (Array.isArray(value)) {
      value.forEach((item) => target.append(key, item));
    } else if (typeof value === "string") {
      target.set(key, value);
    }
  }

  const suffix = target.toString() ? `?${target.toString()}` : "";
  redirect(`/${locale}/app/notifications${suffix}`);
}
