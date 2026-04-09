import "@/styles/globals.css";
import type { ReactNode } from "react";
import { NextIntlClientProvider } from "next-intl";
import { ThemeProvider } from "@/components/providers";

export default async function LocaleLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const currentLocale = locale === "en" ? "en" : "zh";
  const messages = (await import(`@/messages/${currentLocale}.json`)).default;

  return (
    <html lang={currentLocale} suppressHydrationWarning>
      <body className="min-height-screen bg-background text-foreground font-sans antialiased">
        <NextIntlClientProvider messages={messages} locale={currentLocale}>
          <ThemeProvider attribute="data-theme" defaultTheme="dark" disableTransitionOnChange enableSystem forcedTheme="dark">
            {children}
          </ThemeProvider>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
