// 主题：auto / light / dark，持久化在 localStorage。
const KEY = "kwrt.theme";
const ORDER = ["auto", "light", "dark"];
const ICON = { auto: "🌗", light: "☀️", dark: "🌙" };
const LABEL = { auto: "跟随系统", light: "浅色", dark: "深色" };

export function currentTheme() {
  return localStorage.getItem(KEY) || "auto";
}

export function applyTheme(t) {
  const root = document.documentElement;
  if (t === "auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", t);
  localStorage.setItem(KEY, t);
  // 同步按钮显示
  const btn = document.getElementById("btn-theme");
  if (btn) {
    btn.textContent = ICON[t];
    btn.setAttribute("data-tip", `主题：${LABEL[t]}（点击切换 跟随系统 → 浅色 → 深色）`);
  }
}

export function bindThemeButton() {
  const btn = document.getElementById("btn-theme");
  if (!btn) return;
  btn.addEventListener("click", () => {
    const cur = currentTheme();
    const next = ORDER[(ORDER.indexOf(cur) + 1) % ORDER.length];
    applyTheme(next);
  });
  applyTheme(currentTheme());
}
