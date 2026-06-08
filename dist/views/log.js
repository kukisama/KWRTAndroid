import { el, toast } from "../dom.js";
import { api, formatError } from "../api.js";

export default async function mount(root, ctx) {
  const pane = el("pre", { class: "log-pane", tip: "PassWall 实时日志（默认每 3s 刷新）" });
  const lines = el("input", { type: "number", value: 300, min: 50, max: 5000, style: { width: "90px" }, tip: "拉取最近多少行" });
  const auto = el("input", { type: "checkbox", tip: "自动每 3 秒刷新一次" });
  auto.checked = true;
  let timer = 0;

  async function pull() {
    try {
      const t = await api.readLog(ctx.config, Number(lines.value || 300));
      pane.textContent = t || "(暂无日志输出)";
      pane.scrollTop = pane.scrollHeight;
    } catch (e) { pane.textContent = formatError(e); }
  }

  function startTimer() {
    stopTimer();
    if (auto.checked) timer = setInterval(pull, 3000);
  }
  function stopTimer() { if (timer) { clearInterval(timer); timer = 0; } }
  auto.addEventListener("change", startTimer);

  root.append(el("section", { class: "card" }, [
    el("h3", {}, "运行日志"),
    el("div", { class: "row" }, [
      el("label", { class: "inline", tip: "每次拉取的行数" }, ["行数：", lines]),
      el("label", { class: "inline", tip: "勾上自动每 3 秒刷新" }, [auto, " 自动刷新"]),
      el("button", { class: "ghost", tip: "立即拉一次", onclick: pull }, "刷新"),
    ]),
    pane,
  ]));

  pull();
  startTimer();
  return () => stopTimer();
}
