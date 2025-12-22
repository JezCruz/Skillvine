document.addEventListener('DOMContentLoaded', () => {
  function getCookie(name) {
    const matches = document.cookie.match(new RegExp('(?:^|; )' + name.replace(/([.$?*|{}()\[\]\\\/\+^])/g, '\\$1') + '=([^;]*)'));
    return matches ? decodeURIComponent(matches[1]) : undefined;
  }

  function deleteCookie(name) {
    document.cookie = name + '=; Path=/; Max-Age=0';
  }

  function showNotice(message, type = 'info', duration = 3000) {
    let el = document.getElementById('top-notice');
    if (!el) {
      el = document.createElement('div');
      el.id = 'top-notice';
      el.className = 'top-notice alert';
      document.body.appendChild(el);
    }

    // Build alert inner HTML with close button
    el.innerHTML = `<div class="message">${escapeHtml(message)}</div><button class="closebtn" aria-label="close">&times;</button>`;

    // apply type class
    el.classList.remove('success', 'info', 'warning');
    if (type === 'success') el.classList.add('success');
    else if (type === 'info') el.classList.add('info');
    else if (type === 'warning') el.classList.add('warning');

    // show
    el.classList.add('show');
    el.classList.remove('hide');

    // close handler
    const btn = el.querySelector('.closebtn');
    if (btn) {
      btn.onclick = function () {
        el.style.opacity = '0';
        setTimeout(() => { el.remove(); }, 600);
      };
    }

    if (duration > 0) {
      setTimeout(() => {
        if (el) {
          el.style.opacity = '0';
          setTimeout(() => { if (el && el.remove) el.remove(); }, 600);
        }
      }, duration);
    }
  }

  function escapeHtml(unsafe) {
    return unsafe
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/\"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }

  const flash = getCookie('flash');
  if (flash) {
    showNotice(flash, 'success', 2000);
    deleteCookie('flash');
  }
});
