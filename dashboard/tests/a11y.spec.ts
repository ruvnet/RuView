/* axe-core accessibility smoke against the built dashboard.
 * Closes ADR-092 §11.5 — formal axe scan.
 *
 * Runs against `npm run preview` (Vite preview server). Validates each
 * primary view (home / scene / apps / inspector / witness / ghost-murmur)
 * and asserts 0 critical/serious violations.
 */

import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const VIEWS = ['home', 'scene', 'apps', 'inspector', 'witness', 'ghost-murmur'] as const;

test.describe('axe-core a11y smoke', () => {
  for (const view of VIEWS) {
    test(`view: ${view}`, async ({ page }) => {
      await page.goto('/');
      // Dismiss the welcome modal if it auto-shows, then wait until it has left
      // the accessibility tree. A single poll handles every race at once,
      // because each of these was observed as an intermittent failure across
      // chromium/firefox/webkit:
      //   - `nv-app` may not have upgraded yet (no shadowRoot to query);
      //   - Lit may not have upgraded `nv-onboarding` yet, so a `.skip` click
      //     would be a silent no-op;
      //   - `nv-onboarding` decides whether to open inside an async storage
      //     read, so before its `ready` attribute is set "not open" only means
      //     "not decided yet". Treating that as dismissed let the modal open
      //     underneath the scan.
      //   - the modal fades out over 180ms while staying in the DOM. Scanning
      //     inside that window samples a half-transparent panel, and contrast
      //     tooling composites the fading text over the dimmed backdrop into
      //     false failures no user ever sees.
      await page.waitForFunction(() => {
        const app = document.querySelector('nv-app') as (HTMLElement & { shadowRoot: ShadowRoot }) | null;
        if (!app?.shadowRoot) return false;
        const ob = app.shadowRoot.querySelector('nv-onboarding') as HTMLElement | null;
        if (!ob) return true;
        if (!ob.shadowRoot || !ob.hasAttribute('ready')) return false;
        if (ob.hasAttribute('open')) {
          (ob.shadowRoot.querySelector('.skip') as HTMLElement | null)?.click();
          return false;
        }
        return getComputedStyle(ob).visibility === 'hidden';
      });
      // Navigate to the view via the rail button (except for home which is default).
      if (view !== 'home') {
        await page.evaluate((v) => {
          const sr = (document.querySelector('nv-app') as HTMLElement & { shadowRoot: ShadowRoot }).shadowRoot;
          const rail = sr.querySelector('nv-rail') as HTMLElement & { shadowRoot: ShadowRoot };
          const btn = rail.shadowRoot.querySelector(`button[data-id=${v}-btn]`) as HTMLElement | null;
          btn?.click();
        }, view);
        await page.waitForTimeout(300);
      }

      const results = await new AxeBuilder({ page })
        .options({ runOnly: ['wcag2a', 'wcag2aa'] })
        .analyze();

      const critical = results.violations.filter((v) => v.impact === 'critical');
      const serious = results.violations.filter((v) => v.impact === 'serious');

      // Logging the violation summary makes CI failures readable.
      if (critical.length || serious.length) {
        for (const v of [...critical, ...serious]) {
          console.error(`[${view}] ${v.impact} · ${v.id} · ${v.help}`);
          for (const node of v.nodes) console.error(`    ${node.target.join(' >> ')}`);
        }
      }

      expect(critical.length, 'no critical violations').toBe(0);
      expect(serious.length, 'no serious violations').toBe(0);
    });
  }
});
