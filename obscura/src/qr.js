(() => {
  const selectors = ['.Qrcode-qrcode', '.Qrcode-img canvas', '.Qrcode-img img', '.SignFlow-qrcode canvas', 'canvas[alt="二维码"]'];
  for (const selector of selectors) {
    for (const el of document.querySelectorAll(selector)) {
      const r = el.getBoundingClientRect();
      if (r.width < 60 || r.height < 60 || getComputedStyle(el).display === 'none') continue;
      let data = null;
      try {
        if (el.tagName === 'CANVAS') data = el.toDataURL('image/png');
        else if (el.src.startsWith('data:image/png;base64,')) data = el.src;
      } catch (_) {}
      return {data, rect: {x:r.x, y:r.y, width:r.width, height:r.height}};
    }
  }
  const toggle = document.querySelector('.SignFlow-qrcodeTab');
  if (toggle) toggle.click();
  return {};
})()
