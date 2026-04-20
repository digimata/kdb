import type { Metadata } from "next";
import { Geist_Mono, Inter } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import "./globals.css";

import { ScrollArea } from "@/components/ui/scroll-area";
import { Scrollbar } from "@/components/scrollbar";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

const title = "kdb — a knowledge database for your project";
const description =
  "Notes, projects, cycles, and work items — all in your repo, all in one SQLite file, all materialized as markdown. Your graph, not ours.";

export const metadata: Metadata = {
  title,
  description,
  openGraph: {
    title,
    description,
    url: "https://kdb.digimata.dev",
    siteName: "kdb",
  },
  twitter: {
    card: "summary_large_image",
    title,
    description,
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`dark h-full ${inter.variable} ${geistMono.variable} antialiased`}
    >
      <body className="relative h-full overflow-hidden">
        <ScrollArea className="h-screen">{children}</ScrollArea>
        <Scrollbar />
        <Analytics />
      </body>
    </html>
  );
}
