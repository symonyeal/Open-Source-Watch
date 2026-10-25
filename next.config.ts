import type { NextConfig } from "next";

// A snapshot build (NEXT_PUBLIC_SNAPSHOT set, src/lib/snapshot.ts) is a static
// export for GitHub Pages under NEXT_PUBLIC_BASE_PATH; otherwise a self-contained
// server bundle for a slim Docker runtime image.
const snapshot = Boolean(process.env.NEXT_PUBLIC_SNAPSHOT);

const nextConfig: NextConfig = {
  reactStrictMode: true,
  ...(snapshot
    ? { output: "export", basePath: process.env.NEXT_PUBLIC_BASE_PATH, images: { unoptimized: true } }
    : { output: "standalone" }),
  // Disable Next.js fetch cache globally — we handle our own polling intervals
  experimental: {
    serverComponentsHmrCache: false,
  },
};

export default nextConfig;
