import Link from "next/link";

import { firstValue, safeRedirectTarget } from "../../../lib/auth";

type RegisterPageProps = {
  searchParams?: Promise<{
    email?: string | string[];
    error?: string | string[];
    next?: string | string[];
  }>;
};

export default async function RegisterPage({ searchParams }: RegisterPageProps) {
  const params = (await searchParams) ?? {};
  const email = firstValue(params.email) ?? "";
  const error = firstValue(params.error);
  const next = safeRedirectTarget(firstValue(params.next), "/app/dashboard");

  return (
    <main>
      <h1>Register</h1>
      <p>Create your account and verify your email.</p>
      {error ? <p role="alert">{error}</p> : null}
      <form action="/api/auth/register" method="post">
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
              autoComplete="new-password"
              name="password"
              required
              type="password"
            />
          </label>
        </p>
        <button type="submit">Create account</button>
      </form>
      <p>
        Already registered? <Link href="/login">Login</Link>
      </p>
    </main>
  );
}
