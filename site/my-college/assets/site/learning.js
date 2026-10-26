import { layout as baseLayout, esc } from '../app.js';

// The soft router swaps <main> only, so load site CSS on both direct and soft entry.
export async function layout(active) {
  if (!document.getElementById('learning-css')) {
    const link = document.createElement('link');
    link.id = 'learning-css'; link.rel = 'stylesheet'; link.href = '/assets/site/learning.css';
    document.head.append(link);
  }
  const pending = baseLayout(active);
  const nav = document.querySelector('#top [data-k="browse"]');
  if (nav) nav.textContent = '课程目录';
  const user = await pending;
  try {
    sessionStorage.setItem('ludao.top', document.getElementById('top').innerHTML);
    sessionStorage.setItem('ludao.foot', document.getElementById('foot').innerHTML);
  } catch { /* Storage can be disabled. */ }
  return user;
}

export function ancestors(t, node) {
  const path = []; const seen = new Set([node.id]);
  let p = t.byId.get(node.parent);
  while (p && !seen.has(p.id)) { path.unshift(p); seen.add(p.id); p = t.byId.get(p.parent); }
  return path;
}

export function courseCard(node, t) {
  const path = ancestors(t, node);
  return `<article class="course-card"><div class="course-card-body"><div class="course-path">${esc(path.map(p => p.name).join(' / ') || '课程资料')}</div><h2><a href="/n/${node.id}">${esc(node.name)}</a></h2>${node.aliases?.length ? `<p>又名 ${esc(node.aliases.join('、'))}</p>` : ''}<div class="course-card-bottom"><span>${node.count ? `${node.count} 份资料` : '暂无资料'}</span><a href="/n/${node.id}" aria-label="查看${esc(node.name)}课程资料">查看资料 →</a></div></div></article>`;
}
