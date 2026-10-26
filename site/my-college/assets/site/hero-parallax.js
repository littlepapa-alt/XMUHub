// Move only the campus photograph; text, search and the contrast overlay stay still.
let disposePrevious;

export function initHeroParallax() {
  disposePrevious?.();
  const hero = document.querySelector('.campus-hero');
  const main = hero?.closest('main');
  if (!hero || !main) return;

  const controller = new AbortController();
  const { signal } = controller;
  const motion = matchMedia('(prefers-reduced-motion: no-preference) and (hover: hover) and (pointer: fine)');
  let visible = false;
  let frame = 0;
  let x = 0, y = 0, targetX = 0, targetY = 0;
  let previousTime = 0;

  function reset() {
    cancelAnimationFrame(frame);
    frame = previousTime = 0;
    x = y = targetX = targetY = 0;
    hero.style.removeProperty('--photo-x');
    hero.style.removeProperty('--photo-y');
    hero.classList.remove('is-parallax-active');
  }

  function tick(time) {
    frame = 0;
    if (!hero.isConnected) { dispose(); return; }
    if (!motion.matches || !visible || document.hidden) { reset(); return; }
    const delta = previousTime ? Math.min(time - previousTime, 64) : 16;
    previousTime = time;
    const easing = 1 - Math.exp(-delta / 100);
    x += (targetX - x) * easing;
    y += (targetY - y) * easing;
    const settled = Math.abs(targetX - x) < .05 && Math.abs(targetY - y) < .05;
    if (settled) { x = targetX; y = targetY; }
    hero.style.setProperty('--photo-x', `${x.toFixed(2)}px`);
    hero.style.setProperty('--photo-y', `${y.toFixed(2)}px`);
    if (!settled) frame = requestAnimationFrame(tick);
    else { previousTime = 0; hero.classList.remove('is-parallax-active'); }
  }

  function animate() {
    if (frame || !motion.matches || !visible || document.hidden) return;
    hero.classList.add('is-parallax-active');
    frame = requestAnimationFrame(tick);
  }

  hero.addEventListener('pointermove', event => {
    if (event.pointerType !== 'mouse' || !motion.matches || !visible) return;
    const bounds = hero.getBoundingClientRect();
    const clamp = value => Math.max(-1, Math.min(1, value));
    targetX = clamp((event.clientX - bounds.left) / bounds.width * 2 - 1) * 18;
    targetY = clamp((event.clientY - bounds.top) / bounds.height * 2 - 1) * 12;
    animate();
  }, { signal, passive:true });

  function returnToCenter() { targetX = targetY = 0; animate(); }
  hero.addEventListener('pointerleave', returnToCenter, { signal });
  hero.addEventListener('pointercancel', returnToCenter, { signal });
  window.addEventListener('blur', reset, { signal });
  window.addEventListener('pagehide', reset, { signal });
  window.addEventListener('resize', reset, { signal, passive:true });
  document.addEventListener('visibilitychange', reset, { signal });
  motion.addEventListener('change', reset, { signal });

  const visibility = new IntersectionObserver(entries => {
    visible = entries[0].isIntersecting;
    if (!visible) reset();
  });
  visibility.observe(hero);

  // The site's soft router replaces main's children without reloading the document.
  const lifecycle = new MutationObserver(() => { if (!hero.isConnected) dispose(); });
  lifecycle.observe(main, { childList:true });

  function dispose() {
    reset();
    controller.abort();
    visibility.disconnect();
    lifecycle.disconnect();
    if (disposePrevious === dispose) disposePrevious = undefined;
  }
  disposePrevious = dispose;
}
