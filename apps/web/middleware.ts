import createMiddleware from 'next-intl/middleware';

export default createMiddleware({
  // A list of all locales that are supported
  locales: ['en', 'zh'],
  
  // Used when no locale matches
  defaultLocale: 'en',
  localePrefix: 'always'
});

export const config = {
  // Match only internationalized pathnames
  matcher: ['/', '/(zh|en)/:path*', '/((?!api|_next|_static|_vercel|[\\w-]+\\.\\w+).*)']
};
