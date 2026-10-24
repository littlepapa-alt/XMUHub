import { catalog } from './catalog.js';
import { template } from './browse-template.js';
if (new URLSearchParams(location.search).get('pick') === '1') {
  document.querySelector('main').innerHTML = template;
  const script = '/assets/site/tree-picker.js';
  await import(script + (script.includes('?') ? '&' : '?') + 'r=' + Date.now());
} else await catalog();
