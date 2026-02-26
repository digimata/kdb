const GITHUB_API = "https://api.github.com/repos/dremnik/kdb/releases/latest";

/** GET /latest — returns the latest kdb release tag as plain text. */
export async function GET() {
  const res = await fetch(GITHUB_API, {
    headers: { Accept: "application/vnd.github+json" },
    next: { revalidate: 300 },
  });

  if (!res.ok) {
    return new Response("failed to fetch latest release\n", {
      status: 502,
      headers: { "Content-Type": "text/plain" },
    });
  }

  const data = await res.json();
  const tag = data.tag_name;

  if (!tag) {
    return new Response("no tag found\n", {
      status: 502,
      headers: { "Content-Type": "text/plain" },
    });
  }

  return new Response(`${tag}\n`, {
    headers: {
      "Content-Type": "text/plain",
      "Cache-Control": "public, s-maxage=300, stale-while-revalidate=60",
    },
  });
}
