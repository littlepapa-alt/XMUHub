import { api, esc, go, tree } from '../app.js';
import { ancestors, courseCard, layout } from './learning.js';

export async function catalog() {
  const root = document.querySelector('main .catalog');
  const $ = s => root.querySelector(s);
  const params = new URLSearchParams(location.search);
  let q = (params.get('q') || '').trim();
  const state = { grade: params.get('grade') || '', semester: params.get('semester') || '', availability: params.get('availability') || '', sort: params.get('sort') || 'name', view: params.get('view') === 'list' ? 'list' : 'grid', page: Math.max(1, Math.floor(Number(params.get('page'))) || 1) };
  const within = Number(params.get('within')) || 0;
  $('#catalog-search input').value = q;
  $('#resource-tab').href = '/search?' + new URLSearchParams({q, type:'resource', ...(within ? {within} : {})});
  function url(patch = {}) {
    const next = {...state, ...patch};
    const u = new URLSearchParams();
    if (q) u.set('q', q);
    if (within) u.set('within', within);
    for (const [k,v] of Object.entries(next)) if (v && !(k === 'page' && v === 1) && !(k === 'sort' && v === 'name') && !(k === 'view' && v === 'grid')) u.set(k, v);
    return (location.pathname === '/search' ? '/search' : '/browse') + (u.size ? '?' + u : '');
  }
  $('#catalog-search').onsubmit = e => { e.preventDefault(); q = e.target.elements.q.value.trim(); go(url({page:1})); };
  await layout('browse');
  if (!root.isConnected) return;
  try {
    const t = await tree();
    if (!root.isConnected) return;
    let courses = t.list.filter(n => n.kind === 'course' && n.status !== 'merged');
    const paths = new Map(courses.map(n => [n.id, ancestors(t,n)]));
    if (within) {
      courses = courses.filter(n => n.id === within || paths.get(n.id).some(p => p.id === within));
      const scope = t.byId.get(within);
      if (scope) $('.catalog-intro p.muted').textContent = `当前范围：${scope.name}。重置可回到全部课程。`;
    }
    const gradeOf = n => paths.get(n.id).find(p => /大[一二三四]|公共资料/.test(p.name)) || paths.get(n.id)[0];
    const semesterOf = n => paths.get(n.id).find(p => /学期/.test(p.name));
    const grades = new Map(courses.map(n => gradeOf(n)).filter(Boolean).map(n => [String(n.id),n.name]));
    const semesters = [...new Set(courses.map(n => semesterOf(n)?.name).filter(Boolean))].sort((a,b) => a.localeCompare(b,'zh-CN'));
    $('#grade').innerHTML += [...grades].map(([id,name]) => `<option value="${id}">${esc(name)}</option>`).join('');
    $('#semester').innerHTML += semesters.map(name => `<option value="${esc(name)}">${esc(name)}</option>`).join('');
    if (!grades.has(state.grade)) state.grade = '';
    if (!semesters.includes(state.semester)) state.semester = '';
    if (q) {
      // Use the existing index for aliases, Chinese tokenization and pinyin matching.
      const ids = new Set(); let page = 1; let found;
      do {
        found = await api('/search?' + new URLSearchParams({q,type:'course',name_only:'true',page,...(within ? {within} : {})}));
        if (!root.isConnected) return;
        for (const item of found.items) if (item.node) ids.add(item.node.id);
        if (found.total > found.page_size * 50) throw new Error('匹配课程较多，请缩小搜索关键词后重试。');
        page++;
      } while ((page - 1) * found.page_size < found.total && page <= 50);
      courses = courses.filter(n => ids.has(n.id));
    }
    for (const k of ['grade','semester','availability','sort']) {
      $('#' + k).value = state[k];
      $('#' + k).onchange = e => { state[k] = e.target.value; state.page = 1; render(); };
    }
    $('#grid-view').onclick = () => {state.view = 'grid'; render();};
    $('#list-view').onclick = () => {state.view = 'list'; render();};
    $('#catalog-pager').onclick = e => {const b = e.target.closest('[data-page]'); if (b) {state.page = Number(b.dataset.page); render(); root.scrollIntoView({block:'start'});} };
    function render() {
      const items = courses.filter(n => (!state.grade || String(gradeOf(n)?.id) === state.grade) && (!state.semester || semesterOf(n)?.name === state.semester) && (!state.availability || (state.availability === 'ready' ? n.count > 0 : !n.count)));
      items.sort(state.sort === 'count' ? (a,b) => b.count-a.count || a.name.localeCompare(b.name,'zh-CN') : (a,b) => a.name.localeCompare(b.name,'zh-CN'));
      const pages = Math.max(1, Math.ceil(items.length / 12)); state.page = Math.min(state.page, pages);
      $('#result-count').textContent = `${q ? `“${q}” · ` : ''}${items.length} 门课程`;
      const box = $('#course-results'); box.classList.toggle('course-list', state.view === 'list');
      box.innerHTML = items.length ? items.slice((state.page-1)*12, state.page*12).map(n => courseCard(n,t)).join('') : '<div class="empty card"><b>没有符合条件的课程</b><p>试试其他关键词或清除筛选。</p><a href="/browse">查看全部课程</a></div>';
      for (const view of ['grid','list']) { const b = $('#' + view + '-view'); b.setAttribute('aria-pressed', String(view === state.view)); b.classList.toggle('primary', view === state.view); }
      $('#catalog-pager').innerHTML = pages > 1 ? `${state.page > 1 ? `<button class="btn sm" data-page="${state.page-1}">上一页</button>` : ''}<span class="small">第 ${state.page} / ${pages} 页</span>${state.page < pages ? `<button class="btn sm" data-page="${state.page+1}">下一页</button>` : ''}` : '';
      history.replaceState(history.state, '', url());
    }
    render();
  } catch(e) {
    if (!root.isConnected) return;
    $('#result-count').textContent = '课程加载失败';
    $('#course-results').innerHTML = `<div class="notice bad">${esc(e.message)} <button class="btn sm" type="button" data-retry>重新加载</button></div>`;
    $('[data-retry]').onclick = () => location.reload();
  }
}
