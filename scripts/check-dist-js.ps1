# 用 acorn 静态校验 dist/ 下所有 ES Module，及早发现引号嵌套之类的低级错误。
# 用法： pwsh scripts/check-dist-js.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
  if (-not (Test-Path "node_modules/acorn")) {
    Write-Host "==> npm i acorn"
    npm i --silent --no-audit --no-fund acorn | Out-Null
  }
  $code = @'
const acorn = require("acorn");
const fs = require("fs");
const path = require("path");
let bad = 0;
function walk(d) {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p);
    else if (e.name.endsWith(".js")) {
      try {
        acorn.parse(fs.readFileSync(p, "utf8"), { sourceType: "module", ecmaVersion: "latest" });
      } catch (err) {
        bad++;
        console.log("BAD", p, "L" + err.loc.line + ":" + err.loc.column, err.message);
      }
    }
  }
}
walk("dist");
if (bad === 0) console.log("OK: dist/ JS modules all parse");
process.exit(bad === 0 ? 0 : 1);
'@
  node -e $code
  exit $LASTEXITCODE
} finally {
  Pop-Location
}
