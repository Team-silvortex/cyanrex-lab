const { buildContentSecurityPolicy } = require("./config/security-headers");

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  async headers() {
    return [
      {
        source: "/(.*)",
        headers: [
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "X-Frame-Options", value: "DENY" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          {
            key: "Content-Security-Policy",
            value: buildContentSecurityPolicy(process.env.NEXT_PUBLIC_ENGINE_URL),
          },
        ],
      },
    ];
  },
};

module.exports = nextConfig;
