import type { NextConfig } from "next";

// -----------------------
// apps/landing/next.config.ts
//
// const nextConfig    L10
// async rewrites()    L11
// -----------------------

const nextConfig: NextConfig = {
  async rewrites() {
    return [{ source: "/install", destination: "/install.sh" }];
  },
};

export default nextConfig;
