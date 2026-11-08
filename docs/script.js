// ========= Before/After Slider =========
(function () {
  const handle = document.getElementById('sliderHandle');
  const after = document.getElementById('sliderAfter');
  if (!handle || !after) return;

  const frame = handle.parentElement;
  let isDragging = false;

  function move(x) {
    const rect = frame.getBoundingClientRect();
    const pct = Math.max(0, Math.min(100, ((x - rect.left) / rect.width) * 100));
    handle.style.left = pct + '%';
    after.style.clipPath = `inset(0 ${100 - pct}% 0 0)`;
  }

  handle.addEventListener('mousedown', (e) => { isDragging = true; e.preventDefault(); });
  handle.addEventListener('touchstart', (e) => { isDragging = true; }, { passive: true });

  window.addEventListener('mousemove', (e) => { if (isDragging) move(e.clientX); });
  window.addEventListener('touchmove', (e) => { if (isDragging && e.touches[0]) move(e.touches[0].clientX); }, { passive: true });

  window.addEventListener('mouseup', () => isDragging = false);
  window.addEventListener('touchend', () => isDragging = false);

  // Click anywhere on frame to jump
  frame.addEventListener('click', (e) => {
    if (e.target === handle || handle.contains(e.target)) return;
    move(e.clientX);
  });

  // Auto-animate on load for first impression
  let pct = 0;
  const duration = 1400;
  const start = performance.now();
  function animate(now) {
    const t = Math.min(1, (now - start) / duration);
    const eased = 1 - Math.pow(1 - t, 3); // ease-out cubic
    pct = eased * 50;
    handle.style.left = pct + '%';
    after.style.clipPath = `inset(0 ${100 - pct}% 0 0)`;
    if (t < 1) requestAnimationFrame(animate);
  }
  requestAnimationFrame(animate);

  // Pause animation if user interacts before it finishes
  handle.addEventListener('mousedown', () => { cancelAnimationFrame && (isDragging = true); });
})();

// ========= Mobile Nav =========
(function () {
  const toggle = document.querySelector('.nav-toggle');
  const links = document.querySelector('.nav-links');
  if (!toggle || !links) return;

  toggle.addEventListener('click', () => {
    links.style.display = links.style.display === 'flex' ? 'none' : 'flex';
    links.style.flexDirection = 'column';
    links.style.position = 'absolute';
    links.style.top = '100%';
    links.style.left = '0';
    links.style.right = '0';
    links.style.background = 'var(--bg2)';
    links.style.padding = '20px 24px';
    links.style.borderBottom = '1px solid var(--rule)';
  });
})();

// ========= Smooth reveal on scroll =========
(function () {
  const els = document.querySelectorAll('.feature-card, .pricing-card, .compare-col, .step, .download-card');
  if (!('IntersectionObserver' in window)) {
    els.forEach(e => e.style.opacity = 1);
    return;
  }
  const io = new IntersectionObserver((entries) => {
    entries.forEach((entry, i) => {
      if (entry.isIntersecting) {
        setTimeout(() => {
          entry.target.style.opacity = 1;
          entry.target.style.transform = 'translateY(0)';
        }, i * 50);
        io.unobserve(entry.target);
      }
    });
  }, { threshold: 0.1 });

  els.forEach((el, i) => {
    el.style.opacity = 0;
    el.style.transform = 'translateY(16px)';
    el.style.transition = 'opacity .5s ease, transform .5s ease';
    // Stagger: slightly different delay per card
    io.observe(el);
  });
})();

// ========= Update footer year =========
const y = document.querySelector('.footer-bottom p');
if (y) y.innerHTML = y.innerHTML.replace(/20\d{2}/, new Date().getFullYear());
