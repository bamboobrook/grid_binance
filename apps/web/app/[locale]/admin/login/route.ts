import { NextResponse } from "next/server";

export async function GET(request: Request, props: { params: Promise<{ locale: string }> }) {
  const params = await props.params;
  const url = new URL(`/${params.locale}/login`, request.url);
  url.searchParams.set("next", `/${params.locale}/admin/dashboard`);
  return NextResponse.redirect(url, { status: 303 });
}
