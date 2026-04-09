import { redirect } from "next/navigation";

type PageProps = {
  params: Promise<{ locale: string }>;
};

export default async function LocalizedAdminLoginPage({ params }: PageProps) {
  const { locale } = await params;
  redirect(`/${locale}/login?next=/${locale}/admin/dashboard`);
}
