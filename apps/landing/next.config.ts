import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [{ source: "/install", destination: "/install.sh" }];
  },
};

export default nextConfig;
