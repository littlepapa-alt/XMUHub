export const template = `<div class="learning resource-search"><div class="wrap">
  <p class="eyebrow">RESOURCE SEARCH</p><p><a href="/browse">← 返回课程目录</a></p><form class="bigsearch" action="/search" role="search" style="margin:8px 0 18px;max-width:none"><input name="q" id="sq" placeholder="搜索课程名称或拼音首字母" aria-label="搜索"><input type="hidden" name="type" id="stype"><button class="btn primary">搜索</button></form>
  <div class="page-head"><div><h1 id="title">搜索</h1><div class="muted small"><span id="summary"></span> <span id="scope"></span></div></div></div>
  <div class="toolbar"><div class="chips" id="types"></div><span class="grow"></span>
    <select id="within" aria-label="范围"><option value="">全站</option></select>
    <select id="tag" aria-label="资料类型"><option value="">全部类型</option></select></div>
  <div id="results"></div>
  <div class="row" style="justify-content:center;margin-top:16px" id="pager"></div>
</div></div>`;
