// 全局浮动 tooltip：扫描 [data-tip]，鼠标进入显示。
// 子元素冒泡时也找最近祖先的 data-tip，便于给整 row 加提示。
const tip = document.getElementById("tip");
let current = null;
let raf = 0;

function show(el, text) {
  tip.textContent = text;
  tip.hidden = false;
  current = el;
  positionFromEvent(lastEvt);
}
function hide() {
  tip.hidden = true;
  current = null;
}
let lastEvt = null;

function positionFromEvent(e) {
  if (!e) return;
  const pad = 14;
  let x = e.clientX + pad;
  let y = e.clientY + pad;
  const rect = tip.getBoundingClientRect();
  if (x + rect.width + 8 > window.innerWidth) x = e.clientX - rect.width - pad;
  if (y + rect.height + 8 > window.innerHeight) y = e.clientY - rect.height - pad;
  tip.style.left = x + "px";
  tip.style.top = y + "px";
}

document.addEventListener("mousemove", (e) => {
  lastEvt = e;
  const target = e.target.closest?.("[data-tip]");
  if (target) {
    const text = target.getAttribute("data-tip");
    if (target !== current) show(target, text);
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => positionFromEvent(e));
  } else if (current) {
    hide();
  }
});
document.addEventListener("mouseleave", hide);
window.addEventListener("blur", hide);
window.addEventListener("scroll", hide, true);
