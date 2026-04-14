import { NextResponse } from "next/server";
import { publicUrl } from "@/lib/auth";

export async function GET(request: Request, props: { params: Promise<{ locale: string }> }) {
  const params = await props.params;
  const url = publicUrl(request, `/${params.locale}/login`);
  url.searchParams.set("next", `/${params.locale}/admin/dashboard`);
  return NextResponse.redirect(url, { status: 303 });
}
