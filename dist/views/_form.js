// 公用：受控的小型表单工具，每个控件都强制带 data-tip
import { el } from "./dom.js";

export function field(label, tip, control) {
  return el("label", { tip, class: "grow" }, [label, control]);
}

export function text(name, value, tip, extra = {}) {
  const i = el("input", { name, value: value ?? "", tip, ...extra });
  return i;
}

export function number(name, value, tip, extra = {}) {
  return el("input", { name, type: "number", value: value ?? "", tip, ...extra });
}

export function textarea(name, value, tip, extra = {}) {
  const t = el("textarea", { name, tip, ...extra });
  t.value = value ?? "";
  return t;
}

export function checkbox(name, checked, tip, labelText) {
  const c = el("input", { name, type: "checkbox", tip });
  c.checked = !!checked;
  return el("label", { class: "inline", tip }, [c, " " + labelText]);
}

export function select(name, value, options, tip) {
  const s = el("select", { name, tip });
  for (const o of options) {
    const opt = el("option", { value: o.value }, o.label);
    if (String(o.value) === String(value ?? "")) opt.selected = true;
    s.append(opt);
  }
  return s;
}

// 把表单 collect 成 patch（值为 string，null 跳过）
export function collect(form) {
  const out = {};
  for (const elx of form.elements) {
    if (!elx.name) continue;
    if (elx.type === "checkbox") out[elx.name] = elx.checked ? "1" : "0";
    else out[elx.name] = elx.value;
  }
  return out;
}
