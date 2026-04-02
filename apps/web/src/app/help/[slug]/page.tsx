import { notFound, redirect } from "next/navigation";

import { isValidHelpArticle } from "../../../lib/api/help-articles";

export default async function HelpArticlePage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;

  if (!isValidHelpArticle(slug)) {
    notFound();
  }

  redirect(`/app/help?article=${slug}`);
}
