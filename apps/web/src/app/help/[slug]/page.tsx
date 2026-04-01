import Link from "next/link";

function formatSlug(slug: string) {
  return slug
    .split("-")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export default async function HelpArticlePage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  const title = formatSlug(slug);

  return (
    <main>
      <h1>{title}</h1>
      <p>
        Review what happens before access expires, where the reminder appears, and which billing
        page state to check next.
      </p>
      <ul>
        <li>
          <Link href="/app/billing">Billing Center</Link>
        </li>
        <li>
          <Link href="/app/dashboard">User Dashboard</Link>
        </li>
      </ul>
    </main>
  );
}
