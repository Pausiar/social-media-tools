import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const dist = process.env.TRUNK_STAGING_DIR || join(process.cwd(), 'dist');
const htmlPath = join(dist, 'index.html');
let html = readFileSync(htmlPath, 'utf8');
let index = 0;

html = html.replace(/<script type="module">([\s\S]*?)<\/script>/g, (_, body) => {
  const name = `app-loader-${index}.js`;
  writeFileSync(join(dist, name), body, 'utf8');
  index += 1;
  return `<script type="module" src="${name}"></script>`;
});

writeFileSync(htmlPath, html, 'utf8');
console.log(`[csp] externalized ${index} inline module script(s)`);
