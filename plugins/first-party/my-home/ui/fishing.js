/* 池塘挑战的独立运动计算；位置从水底0到水面1，不接触钱包或存档。 */
window.HomeFishing = (() => {
  const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
  function create(grams, seed = Math.floor(Math.random() * 0xffffffff)) {
    let sequence = seed >>> 0;
    const random = () => {
      sequence = (sequence + 0x6d2b79f5) >>> 0;
      let value = Math.imul(sequence ^ sequence >>> 15, sequence | 1);
      value ^= value + Math.imul(value ^ value >>> 7, value | 61);
      return ((value ^ value >>> 14) >>> 0) / 4294967296;
    };
    const size = Math.sqrt(clamp(grams / 15000, 0, 1));
    return {
      grams, random, size, span: .43 - .25 * size,
      target: .4 + random() * .2, targetVelocity: 0, goal: .5,
      turnIn: .2 + random() * .4, spring: 9 + random() * 5,
      hook: .5, velocity: 0, progress: .25, elapsed: 0,
      inside: true, status: 'playing',
    };
  }
  function lift(game) {
    if (game.status === 'playing') game.velocity = clamp(Math.max(.16, game.velocity + .23), -.8, .85);
  }
  function step(game, seconds, holding = false) {
    if (game.status !== 'playing' || !Number.isFinite(seconds) || seconds <= 0) return game;
    let remaining = Math.min(seconds, .1);
    while (remaining > 0 && game.status === 'playing') {
      const dt = Math.min(remaining, 1 / 120); remaining -= dt;
      game.elapsed += dt; game.turnIn -= dt;
      const edge = game.span / 2 + .025;
      if (game.turnIn <= 0) {
        game.goal = edge + game.random() * (1 - edge * 2);
        game.turnIn = .55 + game.random() * (1.2 - game.size * .35);
        game.spring = 7 + game.random() * 9 + game.size * 5;
      }
      const speed = .15 + game.size * .35;
      game.targetVelocity += ((game.goal - game.target) * game.spring - game.targetVelocity * 5) * dt;
      game.targetVelocity = clamp(game.targetVelocity, -speed, speed);
      game.target = clamp(game.target + game.targetVelocity * dt, edge, 1 - edge);
      game.velocity = clamp(game.velocity + (holding ? 1.4 : -1.05) * dt, -.8, .85);
      game.hook += game.velocity * dt;
      if (game.hook < .025) { game.hook = .025; game.velocity = Math.max(0, -game.velocity * .2); }
      if (game.hook > .975) { game.hook = .975; game.velocity = Math.min(0, -game.velocity * .2); }
      game.inside = Math.abs(game.hook - game.target) <= game.span / 2;
      const gain = game.inside ? .145 : game.elapsed < 1.5 ? 0 : -.10;
      game.progress = clamp(game.progress + gain * dt, 0, 1);
      if (game.progress >= 1) game.status = 'caught';
      else if (game.progress <= 0) game.status = 'escaped';
    }
    return game;
  }
  function open({ grams, onResult, onClose }) {
    const game = create(grams);
    const dialog = document.createElement('dialog');
    dialog.className = 'fishing-dialog';
    dialog.setAttribute('aria-labelledby', 'fishing-title');
    const sizeName = grams < 1500 ? '小鱼' : grams < 6000 ? '中鱼' : '大鱼';
    dialog.innerHTML = `<header class="fishing-heading"><div><span class="fishing-eyebrow">池塘时光 · ${sizeName}</span><h2 id="fishing-title">鱼儿上钩了</h2></div><span class="fishing-illustration" aria-hidden="true">🎣</span></header>
      <p class="fishing-help">点按让鱼钩上抬，松开自然下落。跟住绿色活动区，攒满进度就能收鱼。</p>
      <div class="fishing-stage" data-inside="true"><div class="fishing-water"><span class="fishing-depth">水面</span><div class="fishing-lane"><div class="fishing-target"><span aria-hidden="true">🐟</span></div><div class="fishing-hook"><span aria-hidden="true">🪝</span></div></div><span class="fishing-depth fishing-depth--bottom">水底</span></div>
      <div class="fishing-meter-wrap"><span>收鱼</span><div class="fishing-meter" role="progressbar" aria-label="钓鱼成功进度" aria-valuemin="0" aria-valuemax="100" aria-valuenow="25"><i></i></div><output>25%</output></div></div>
      <div class="fishing-feedback" role="status">稳住鱼钩，别让鱼儿溜走</div>
      <div class="fishing-result" hidden></div>
      <button type="button" class="fishing-lift">按住或连点提竿 <kbd>Space / ↑</kbd></button>
      <footer class="fishing-footer"><span>大鱼活动区更短 · 放弃不退本次消耗</span><button type="button" class="fishing-exit">放弃本次</button></footer>`;
    document.body.append(dialog);
    const control = dialog.querySelector('.fishing-lift'), exit = dialog.querySelector('.fishing-exit');
    const lane = dialog.querySelector('.fishing-lane'), target = dialog.querySelector('.fishing-target'), hook = dialog.querySelector('.fishing-hook');
    const meter = dialog.querySelector('.fishing-meter'), fill = meter.querySelector('i'), output = dialog.querySelector('output');
    const feedback = dialog.querySelector('.fishing-feedback'), result = dialog.querySelector('.fishing-result');
    let holding = false, closed = false, finishing = false, frame = 0, last = performance.now(), height = 240, lastLabel = 0;
    const measure = () => { height = lane.clientHeight || 240; paint(); };
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    const paint = () => {
      target.style.height = `${game.span * height}px`;
      target.style.transform = `translate3d(0,${(1 - game.target - game.span / 2) * height}px,0)`;
      hook.style.transform = `translate3d(0,${(1 - game.hook) * height}px,0)`;
      fill.style.transform = `scaleY(${game.progress})`;
      dialog.querySelector('.fishing-stage').dataset.inside = String(game.inside);
      const percent = Math.round(game.progress * 100);
      output.textContent = `${percent}%`; meter.setAttribute('aria-valuenow', String(percent));
      control.dataset.held = String(holding);
    };
    const release = () => { holding = false; };
    const cleanup = () => {
      if (closed) return; closed = true; release(); cancelAnimationFrame(frame); observer?.disconnect();
      window.removeEventListener('blur', release); window.removeEventListener('pointerup', release);
      dialog.close(); dialog.remove(); onClose?.();
    };
    const saveResult = async caught => {
      finishing = true; release(); cancelAnimationFrame(frame); control.disabled = true; exit.disabled = true;
      feedback.textContent = caught ? '正在收鱼…' : '这次鱼儿溜走了';
      let saved = false;
      try { saved = await onResult(caught); } catch { saved = false; }
      if (closed) return;
      exit.disabled = false; dialog.querySelector('.fishing-water').append(result); result.hidden = false; dialog.dataset.result = caught ? 'caught' : 'escaped';
      if (!saved) {
        result.innerHTML = '<strong>鱼已上钩，鱼获尚未确认保存</strong><p>先别关闭，可重试保存；不会再次扣入场费。</p>';
        control.disabled = false; control.textContent = '重试保存鱼获';
        control.onclick = () => { control.onclick = null; void saveResult(caught); };
        feedback.textContent = '保存遇到问题'; exit.textContent = '关闭';
      } else {
        result.innerHTML = caught ? `<span aria-hidden="true">🐟</span><strong>钓上来了！</strong><p>${grams.toLocaleString('zh-CN')} 克鱼 · 已放入鱼获</p>` : '<span aria-hidden="true">〰</span><strong>鱼儿脱钩了</strong><p>轻轻连点，下一次再跟紧一点。</p>';
        feedback.textContent = caught ? '收获时刻' : '下次再来';
        dialog.querySelector('h2').textContent = caught ? '这条鱼归你啦' : '鱼儿游远了';
        dialog.querySelector('.fishing-footer span').textContent = caught ? '回到池塘可以继续抛竿，或出售鱼获' : '本次消耗已保留，回池塘再试试';
        control.disabled = false; control.textContent = caught ? '带着鱼获回去' : '回池塘再试试'; control.onclick = cleanup;
        exit.textContent = '收好鱼竿';
      }
    };
    const tick = now => {
      if (closed || finishing) return;
      if (!document.hidden) step(game, (now - last) / 1000, holding); last = now; paint();
      if (now - lastLabel > 700) { feedback.textContent = game.inside ? '跟住了，正在积攒收鱼进度' : '跟着绿色活动区，轻轻调整'; lastLabel = now; }
      if (game.status !== 'playing') { void saveResult(game.status === 'caught'); return; }
      frame = requestAnimationFrame(tick);
    };
    control.addEventListener('pointerdown', event => {
      if (event.button !== 0 || finishing) return; event.preventDefault();
      holding = true; lift(game); control.setPointerCapture?.(event.pointerId); paint();
    });
    control.addEventListener('pointerup', release); control.addEventListener('pointercancel', release); control.addEventListener('lostpointercapture', release);
    control.addEventListener('click', event => { if (!finishing && event.detail === 0) { lift(game); paint(); } });
    dialog.addEventListener('keydown', event => {
      if (finishing || event.target === exit || ![' ', 'ArrowUp'].includes(event.key)) return;
      event.preventDefault(); if (!event.repeat) { holding = true; lift(game); paint(); }
    });
    dialog.addEventListener('keyup', event => { if ([' ', 'ArrowUp'].includes(event.key)) { event.preventDefault(); release(); } });
    dialog.addEventListener('cancel', event => { event.preventDefault(); if (!exit.disabled) exit.click(); });
    exit.addEventListener('click', () => { if (!finishing) void onResult(false); cleanup(); });
    window.addEventListener('blur', release); window.addEventListener('pointerup', release);
    dialog.showModal(); observer?.observe(lane); measure(); control.focus({ preventScroll: true }); frame = requestAnimationFrame(tick);
    return { close: cleanup, game };
  }
  return Object.freeze({ create, lift, step, open });
})();
