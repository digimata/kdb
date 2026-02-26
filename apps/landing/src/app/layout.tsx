import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";

// -------------------------------------------
// apps/landing/app/layout.tsx
//
// const geistSans                         L15
// const geistMono                         L20
// export const metadata                   L25
// export default function RootLayout()    L37
// children                                L40
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
      </body>
    </html>
  );
}
