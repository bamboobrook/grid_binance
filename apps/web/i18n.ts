import { getRequestConfig } from 'next-intl/server';
import { notFound } from 'next/navigation';

const locales = ['en', 'zh'];

export default getRequestConfig(async ({ locale }) => {
  const currentLocale = locale || 'en';
  console.log('Locale received in getRequestConfig:', currentLocale);
  if (!locales.includes(currentLocale as any)) {
    console.log('Locale not found, throwing notFound() for locale:', currentLocale);
    notFound();
  }

  return {
    locale: currentLocale as string,
    messages: (await import(`./messages/${currentLocale}.json`)).default
  };
});
