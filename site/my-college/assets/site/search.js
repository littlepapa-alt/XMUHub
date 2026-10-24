import { catalog } from './catalog.js';
import { template } from './search-template.js';
if (new URLSearchParams(location.search).get('type') === 'resource') {
  document.querySelector('main').innerHTML = template;
  const script = '/assets/site/resource-search.js';
  await import(script + (script.includes('?') ? '&' : '?') + 'r=' + Date.now());
} else await catalog();
