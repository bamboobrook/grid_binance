import Link from "next/link";

import { firstValue, safeRedirectTarget } from "../../../lib/auth";

type LoginPageProps = {
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
  }>;
};

export default async function LoginPage({ searchParams }: LoginPageProps) {
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const next = safeRedirectTarget(firstValue(params.next), "/app/dashboard");

  return (
    <main>
      <h1>Login</h1>
      <p>Sign in to access your trading workspace.</p>
      {error ? <p role="alert">{error}</p> : null}
      <form action="/api/auth/login" method="post">
        <input type="hidden" name="next" value={next} />
        <p>
          <label>
            Email
            <input
              autoComplete="email"
              defaultValue={email}
              name="email"
              required
              type="email"
            />
          </label>
        </p>
        <p>
          <label>
            Password
            <input
              autoComplete="current-password"
              name="password"
              required
              type="password"
            />
          </label>
        </p>
        <button type="submit">Sign in</button>
      </form>
      <p>
        Need an account? <Link href="/register">Register</Link>
      </p>
    </main>
  );
}
