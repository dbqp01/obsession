import { z } from 'zod';
import { chromium } from 'playwright-core';
import { WorkflowError } from './errors.mjs';
import { temuProfileDir } from './profile.mjs';

const searchInput = z.object({ query: z.string().trim().min(1).max(200), limit: z.number().int().min(1).max(30).default(10) }).strict();
const productUrl = z.string().url().refine(value => {
  const url = new URL(value);
  return url.protocol === 'https:' && !url.port && !url.username && !url.password && ['temu.com', 'www.temu.com'].includes(url.hostname) && (url.searchParams.has('goods_id') || /(?:goods|g-\d|product)/i.test(url.pathname));
}, 'URL must point to a Temu product');
const detailInput = z.object({ url: productUrl }).strict();
export const temuSearchInput = searchInput;
export const temuDetailInput = detailInput;
const origin = 'https://www.temu.com';
const needsLogin = async page => /login/i.test(page.url()) || page.getByRole('heading', { name: /iniciar sesión|sign in|log in/i }).count().then(Boolean);

export function createTemuRunner({ profile = temuProfileDir() } = {}) {
  let context;
  let page;
  let busy = Promise.resolve();
  let closed = false;

  async function requireLogin(current) {
    if (!await needsLogin(current)) return;
    await context?.close().catch(() => {});
    context = null;
    page = null;
    throw new WorkflowError('LOGIN_REQUIRED', 401);
  }

  async function ready() {
    if (closed) throw new WorkflowError('SESSION_CLOSED', 503);
    if (!context) {
      context = await chromium.launchPersistentContext(profile, { channel: 'chrome', headless: true, viewport: { width: 1280, height: 900 } });
      page = context.pages()[0] ?? await context.newPage();
      await page.goto(origin, { waitUntil: 'domcontentloaded', timeout: 30000 });
      await requireLogin(page);
    }
    return page;
  }

  async function run(task) {
    const previous = busy;
    let release;
    busy = new Promise(resolve => { release = resolve; });
    await previous;
    try {
      const current = await ready();
      await requireLogin(current);
      return await task(current);
    } finally { release(); }
  }

  return {
    async search(raw) {
      const { query, limit } = searchInput.parse(raw);
      return run(async current => {
        const search = current.locator('input[type="search"]').first();
        await search.fill(query);
        await search.press('Enter');
        await current.waitForLoadState('domcontentloaded');
        await current.waitForTimeout(1200);
        await requireLogin(current);
        return current.evaluate(({ limit, query }) => {
          // shortcut: Temu changes product markup; when selectors stop matching, refresh them from the live page.
          const normalize = value => value.normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase();
          const ignored = new Set(['con', 'de', 'del', 'el', 'en', 'la', 'las', 'los', 'para', 'por', 'un', 'una', 'y', 'a', 'the', 'for', 'with', 'and']);
          const terms = normalize(query).match(/[a-z0-9]+/g)?.filter(term => term.length > 1 && !ignored.has(term)) ?? [];
          const minMatches = Math.max(1, Math.ceil(terms.length * 0.6));
          const promo = /^(?:más vendido|mas vendido|oferta|descuento|env[ií]o gratis|flash sale|best seller|hot sale|\d+%|\d+(?:\.\d+)?\s*(?:estrellas?|reviews?))/i;
          const cleanTitle = link => {
            const image = link.querySelector('img[alt]');
            const candidates = [link.getAttribute('aria-label'), link.getAttribute('title'), image?.alt, link.innerText];
            for (const candidate of candidates) {
              const title = candidate?.split(/\n+/).map(line => line.replace(/(?:S\/|[$€£])\s?\d[\d,.]*/g, '').replace(/\s+/g, ' ').trim()).find(line => line.length >= 8 && !promo.test(line));
              if (title) return title.slice(0, 240);
            }
            return '';
          };
          const links = [...document.querySelectorAll('a[href]')];
          const seen = new Set();
          return links.flatMap((link, index) => {
            const url = link.href;
            const product = new URL(url);
            if (!['temu.com', 'www.temu.com'].includes(product.hostname) || !(product.searchParams.has('goods_id') || /(?:goods|g-\d|product)/i.test(product.pathname)) || seen.has(url)) return [];
            let card = link;
            for (let parent = link.parentElement, depth = 0; parent && parent !== document.body && depth < 6; parent = parent.parentElement, depth++) {
              if (parent.querySelector('img[src]') && /(?:S\/|[$€£])\s?\d[\d,.]*/.test(parent.innerText)) { card = parent; break; }
            }
            const title = cleanTitle(link);
            const matched = new Set(normalize(title).match(/[a-z0-9]+/g)?.filter(term => terms.includes(term)) ?? []);
            if (!title || matched.size < minMatches) return [];
            seen.add(url);
            const relevance = matched.size / Math.max(terms.length, 1) + (normalize(title).includes(normalize(query)) ? 1 : 0);
            return [{ title, price: card.innerText.match(/(?:S\/|[$€£])\s?\d[\d,.]*/)?.[0] ?? null, image: card.querySelector('img[src]')?.src ?? null, url, relevance, index }];
          }).sort((a, b) => b.relevance - a.relevance || a.index - b.index).slice(0, limit).map(({ relevance, index, ...product }) => product);
        }, { limit, query });
      });
    },
    async detail(raw) {
      const { url } = detailInput.parse(raw);
      return run(async current => {
        await current.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 });
        await requireLogin(current);
        await current.waitForTimeout(800);
        return current.evaluate(() => ({
          title: document.querySelector('h1')?.innerText?.trim() ?? document.querySelector('meta[property="og:title"]')?.content ?? document.title,
          price: document.querySelector('[class*="price" i]')?.innerText?.trim() ?? document.body.innerText.match(/(?:S\/|[$€£])\s?\d[\d,.]*/)?.[0] ?? null,
          description: document.querySelector('meta[name="description"]')?.content ?? null,
          image: document.querySelector('meta[property="og:image"]')?.content ?? null,
          url: location.href,
        }));
      });
    },
    async close() {
      closed = true;
      await context?.close().catch(() => {});
      context = null;
    },
  };
}
