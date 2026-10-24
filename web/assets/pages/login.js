import { api, forgetMe, layout, loadTurnstile, meta, pwToggle, qs, toast, $ } from '../app.js';

pwToggle($('#password'), $('#password2'));

layout('me').then((u) => { if (u) location.replace(next()); });

function next() {
  const n = qs.get('next') || '/me';
  // Only same-origin targets (open-redirect guard); the URL parser handles //host, /\host, tabs etc.
  try {
    const u = new URL(n, location.origin);
    return u.origin === location.origin ? u.pathname + u.search + u.hash : '/me';
  } catch { return '/me'; }
}

let mode = qs.get('mode') || 'login';

function setMode(m) {
  mode = m;
  document.querySelectorAll('#tabs button').forEach((b) => b.classList.toggle('on', b.dataset.t === m));
  $('#codebox').hidden = m === 'login';
  $('#nickbox').hidden = m !== 'register';
  $('#consentbox').hidden = m !== 'register';
  $('#pw2box').hidden = m === 'login';
  $('#password2').required = m !== 'login';
  $('#pwlabel').textContent = m === 'login' ? '密码' : m === 'reset' ? '新密码（至少 8 位）' : '设置密码（至少 8 位）';
  $('#password').autocomplete = m === 'login' ? 'current-password' : 'new-password';
  $('#go').textContent = { login: '登录', register: '注册并登录', reset: '重置密码并登录' }[m];
  $('#msg').innerHTML = '';
  mailMessage('');
}

$('#tabs').onclick = (e) => { const b = e.target.closest('button'); if (b) setMode(b.dataset.t); };

let cooldown = 0;
function tick() {
  const b = $('#sendcode');
  if (cooldown > 0) { b.disabled = true; b.textContent = `${cooldown} 秒`; cooldown--; setTimeout(tick, 1000); }
  else { b.disabled = false; b.textContent = '获取验证码'; }
}

// Signing up with an uncommon mail domain takes a Cloudflare Turnstile check first: the
// server answers with its site key, the widget appears, and passing it sends the code.
let captchaToken = '';
let captchaWidget = null;
async function showCaptcha(sitekey) {
  $('#captchabox').hidden = false;
  const ts = await loadTurnstile();
  if (captchaWidget !== null) { ts.reset(captchaWidget); return; }
  captchaWidget = ts.render('#captcha', {
    sitekey,
    language: 'zh-cn',
    callback: (token) => { captchaToken = token; sendCode(); },
    'expired-callback': () => { captchaToken = ''; },
    'error-callback': () => { captchaToken = ''; },
  });
}

/** Signing up needs the privacy policy agreed to first (before the address is even mailed). */
function consented() {
  if (mode !== 'register' || $('#consent').checked) return true;
  toast('请先阅读并勾选同意使用须知和隐私政策', true);
  return false;
}

function mailMessage(message, bad = false) {
  const box = $('#mailmsg');
  box.hidden = !message;
  box.className = bad ? 'notice bad' : 'notice';
  box.textContent = message;
}

function validEmail() {
  const input = $('#email');
  input.value = input.value.trim();
  const email = input.value;
  const format = /^[a-z\d_+-]+(?:\.[a-z\d_+-]+)*@(?:[a-z\d](?:[a-z\d-]*[a-z\d])?\.)+[a-z\d](?:[a-z\d-]*[a-z\d])?$/i;
  const valid = email.length <= 100 && email.split('@')[0].length <= 64
    && email.split('@')[1]?.split('.').every((label) => label.length <= 63) && format.test(email);
  input.setAttribute('aria-invalid', String(!valid));
  if (!valid) {
    mailMessage(email ? '邮箱格式不正确，请检查后重试' : '请填写邮箱地址', true);
    input.focus();
  }
  return valid;
}

$('#email').oninput = () => {
  $('#email').removeAttribute('aria-invalid');
  mailMessage('');
};
$('#email').addEventListener('invalid', () => { validEmail(); });

async function sendCode() {
  if (!validEmail()) return;
  const email = $('#email').value.trim();
  if (!consented()) return;
  mailMessage('正在发送验证码，请稍候…');
  $('#sendcode').disabled = true;
  try {
    await api('/auth/code', { method: 'POST', body: { email, purpose: mode === 'reset' ? 'reset' : 'register', captcha: captchaToken } });
    mailMessage('邮件已提交发送，请查收验证码（也请查看垃圾邮件）。若未收到，请确认邮箱真实可用。');
    $('#captchabox').hidden = true;
    cooldown = 60;
    tick();
  } catch (e) {
    mailMessage(e.message, true);
    $('#sendcode').disabled = false;
    // A token works once: a new attempt needs a fresh check.
    captchaToken = '';
    if (e.data && e.data.captcha) {
      try { await showCaptcha(e.data.captcha); } catch (err) { toast(err.message, true); }
      if (!e.message.includes('需要先完成')) toast(e.message, true);
      return;
    }
    toast(e.message, true);
  }
}
$('#sendcode').onclick = sendCode;

$('#f').onsubmit = async (e) => {
  e.preventDefault();
  if (!validEmail()) return;
  if (mode !== 'login' && $('#password').value !== $('#password2').value) {
    $('#msg').innerHTML = '<div class="notice bad">两次输入的密码不一致</div>';
    return;
  }
  if (!consented()) return;
  const body = { email: $('#email').value.trim(), password: $('#password').value };
  if (mode !== 'login') body.code = $('#code').value.trim();
  if (mode === 'register') body.nickname = $('#nickname').value.trim();
  $('#go').disabled = true;
  try {
    await api(`/auth/${mode}`, { method: 'POST', body });
    forgetMe();
    location.replace(next());
  } catch (err) {
    $('#msg').innerHTML = `<div class="notice bad">${err.message.replace(/[<>&]/g, '')}</div>`;
  } finally {
    $('#go').disabled = false;
  }
};

meta().then((m) => {
  if (!m.mail) {
    for (const t of ['register', 'reset']) document.querySelector(`#tabs [data-t="${t}"]`).title = '邮件服务未开启';
  }
});
setMode(mode);
