import { DOMParser } from '@xmldom/xmldom';

export async function fetchWithTimeout(
  url: string,
  options: RequestInit & { timeout?: number } = {}
): Promise<Response> {
  const { timeout = 10000, ...fetchOptions } = options;

  // Open-Source-Watch seam: with ENGINE_URL set, upstream URLs are answered by the
  // engine's recording proxy, so what a route shows is what the engine stored.
  const engine = process.env.ENGINE_URL;
  if (engine && /^https?:\/\//.test(url) && !url.startsWith(engine)) {
    try {
      const res = await fetchWithTimeout(`${engine}/fetch?url=${encodeURIComponent(url)}`, options);
      if (res.status !== 502) return res;
    } catch {
      // engine unreachable: fall through to the direct fetch
    }
    // 502: the engine has no response for this URL; fetch it directly
  }

  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);

  try {
    const response = await fetch(url, {
      ...fetchOptions,
      signal: controller.signal,
      cache: 'no-store',
    });
    return response;
  } catch (err) {
    if (err instanceof DOMException && err.name === 'AbortError') {
      throw new Error(`Fetch timeout after ${timeout}ms: ${url}`);
    }
    throw err;
  } finally {
    clearTimeout(id);
  }
}

export function parseXML(text: string): Document {
  const parser = new DOMParser();
  return parser.parseFromString(text, 'text/xml');
}

export function getTextContent(element: Element, tagName: string): string {
  const el = element.getElementsByTagName(tagName)[0];
  return el?.textContent || '';
}
