import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import "./globals.css";

// -------------------------------------------
// qmd/apps/landing/src/app/layout.tsx
//
// const geistSans                         L16
// const geistMono                         L21
// export const metadata                   L26
// export default function RootLayout()    L38
// children                                L41
// -------------------------------------------

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "kdb — code navigation for AI agents",
  description:
    "The fastest way for agents to navigate code and knowledge bases. Built with Rust.",
  openGraph: {
    title: "kdb — code navigation for AI agents",
    description:
      "The fastest way for agents to navigate code and knowledge bases. Built with Rust.",
    url: "https://kdb.kernl.sh",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={`${geistSans.variable} ${geistMono.variable}`}>
        {children}
        <Analytics />
      </body>
    </html>
  );
}
