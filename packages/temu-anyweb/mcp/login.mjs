import { createInterface } from 'node:readline/promises';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { WorkflowError } from './errors.mjs';
import { temuProfileDir } from './profile.mjs';

const origin = 'https://www.temu.com';
const loginVisible = page => /login/i.test(page.url()) || page.getByRole('heading', { name: /iniciar sesión|sign in|log in/i }).count().then(Boolean);

export async function loginTemu(env = process.env) {
  if (!process.stdin.isTTY) throw new WorkflowError('TTY_REQUIRED', 409);
  const profile = temuProfileDir(env);
  const chrome = env.WEB2API_CHROME_PATH || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
  const browser = spawn(chrome, [`--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', origin], { detached: true, stdio: 'ignore' });
  browser.unref();
  const rl = createInterface({ input: process.stdin, output: process.stderr });
  try {
    await rl.question('Inicia sesión en la ventana normal de Chrome, ciérrala y pulsa Enter: ');
  } finally {
    rl.close();
  }
  const { chromium } = await import('playwright-core');
  const context = await chromium.launchPersistentContext(profile, { channel: 'chrome', headless: true, viewport: { width: 1280, height: 800 } });
  try {
    const page = context.pages()[0] ?? await context.newPage();
    await page.goto(origin, { waitUntil: 'domcontentloaded', timeout: 30000 });
    if (await loginVisible(page)) throw new WorkflowError('LOGIN_REQUIRED', 401);
    return { site: origin, authenticated: true, profile };
  } finally {
    await context.close().catch(() => {});
  }
}

// Run directly (`node login.mjs`): Obsession's Connections panel starts it in a console.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  loginTemu().then(result => console.log(JSON.stringify(result)), error => {
    console.error(JSON.stringify({ error: error.code ?? 'INTERNAL_ERROR' }));
    process.exitCode = 1;
  });
}
