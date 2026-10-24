import { api, esc, loginUrl, meta, resourceItem, tree } from '../app.js';
import { courseCard, layout } from './learning.js';
const root = document.querySelector('main .learning-home');
const $ = s => root.querySelector(s);
const user = await layout('home');
if (root.isConnected) {
  meta().then(m => {
    if (root.isConnected) $('#stats').innerHTML = `<span><b>${m.stats.resources}</b> 份共享资料</span><span><b>${m.stats.nodes}</b> 门已收录课程</span><span><b>${m.stats.downloads}</b> 次学习与下载</span>`;
  }).catch(() => { if (root.isConnected) $('#stats').textContent = '课程资料，由每一位同学共同积累。'; });
  if (!user) {
    $('#home-courses').innerHTML = `<div class="card login-card"><h3>用邮箱登录，开启课程资料库</h3><p class="muted">登录后即可浏览课程、预览资料与下载，也可以分享自己的学习笔记。</p><a class="btn primary" href="${loginUrl()}">登录 / 注册</a></div>`;
    $('.home-recent').hidden = true;
  } else {
    tree().then(t => {
      if (!root.isConnected) return;
      const courses = t.list.filter(n => n.kind === 'course' && n.status !== 'merged').sort((a,b) => b.count-a.count || a.id-b.id);
      $('#home-courses').innerHTML = courses.slice(0,6).map(n => courseCard(n,t)).join('') || '<div class="empty card">课程正在整理，欢迎分享第一份资料。</div>';
    }).catch(e => {if(root.isConnected) $('#home-courses').innerHTML = `<div class="notice bad">${esc(e.message)}</div>`;});
    api('/recent').then(rs => {if(root.isConnected) $('#recent').innerHTML = rs.length ? rs.slice(0,5).map(r => resourceItem(r)).join('') : '<div class="empty">暂无资料</div>';}).catch(e => {if(root.isConnected) $('#recent').innerHTML = `<div class="notice bad">${esc(e.message)}</div>`;});
  }
}
