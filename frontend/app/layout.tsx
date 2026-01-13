import { AuthProvider } from "../lib/auth-context";
import { AppThemeProvider } from "../lib/theme";
import { I18nProvider } from "../lib/i18n";
import { LanguageSwitcher } from "../components/LanguageSwitcher";
import { QueryProvider } from "../lib/query-provider";

export const metadata = {
  title: "Rust Web3 Risk Platform",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body style={{ margin: 0 }}>
        <AppThemeProvider>
          <I18nProvider>
            <QueryProvider>
              <AuthProvider>{children}</AuthProvider>
            </QueryProvider>
            <LanguageSwitcher />
          </I18nProvider>
        </AppThemeProvider>
      </body>
    </html>
  );
}
