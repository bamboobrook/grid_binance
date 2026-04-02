import { notFound, redirect } from "next/navigation";

const validHelpSlugs = new Set(["expiry-reminder"]);

export default async function HelpArticlePage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;

  if (!validHelpSlugs.has(slug)) {
    notFound();
  }

  redirect(`/app/help?article=${slug}`);
}
