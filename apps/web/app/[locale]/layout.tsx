import "@/styles/globals.css";
import type { ReactNode } from "react";
import { NextIntlClientProvider } from 'next-intl';
import { getMessages } from 'next-intl/server';
import { ThemeProvider } from "@/components/providers";

export default async function LocaleLayout({
  children,
  params
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  let messages;
  try {
    messages = await getMessages();
  } catch (e) {
    console.error('Failed to get messages:', e);
    messages = {};
  }

  return (
    <html lang={locale} suppressHydrationWarning>
      <body className="min-height-screen bg-background text-foreground font-sans antialiased">
        <NextIntlClientProvider messages={messages} locale={locale}>
          <ThemeProvider
            attribute="data-theme"
            defaultTheme="system"
            enableSystem
            disableTransitionOnChange
          >
            {children}
          </ThemeProvider>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
