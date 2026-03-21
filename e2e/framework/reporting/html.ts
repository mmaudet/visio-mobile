import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import type { RunResult, SuiteResult, ScenarioResult } from "../participants/types.js";

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function statusBadge(status: "pass" | "fail" | "skip"): string {
  const colors: Record<string, string> = {
    pass: "#22c55e",
    fail: "#ef4444",
    skip: "#eab308",
  };
  const bg = colors[status] ?? "#888";
  return `<span class="badge" style="background:${bg}">${status.toUpperCase()}</span>`;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function renderScenario(scenario: ScenarioResult): string {
  const assertionsHtml =
    scenario.assertions.length > 0
      ? `<table class="assertions">
          <thead><tr><th>Assertion</th><th>Platform</th><th>Status</th><th>Duration</th><th>Error</th></tr></thead>
          <tbody>${scenario.assertions
            .map(
              (a) =>
                `<tr>
                  <td>${escapeHtml(a.description)}</td>
                  <td>${escapeHtml(a.platform)}</td>
                  <td>${statusBadge(a.status)}</td>
                  <td>${formatDuration(a.durationMs)}</td>
                  <td>${a.error ? escapeHtml(a.error) : ""}</td>
                </tr>`,
            )
            .join("")}
          </tbody>
        </table>`
      : "";

  const screenshotsHtml =
    scenario.screenshots.length > 0
      ? `<div class="screenshots">${scenario.screenshots
          .map(
            (s) =>
              `<a href="../screenshots/${escapeHtml(s)}" target="_blank"><img src="../screenshots/${escapeHtml(s)}" alt="${escapeHtml(s)}" class="thumb" /></a>`,
          )
          .join("")}</div>`
      : "";

  const errorHtml = scenario.error
    ? `<pre class="error">${escapeHtml(scenario.error)}</pre>`
    : "";

  return `
    <div class="scenario" data-status="${scenario.status}">
      <div class="scenario-header">
        ${statusBadge(scenario.status)}
        <strong>${escapeHtml(scenario.name)}</strong>
        <span class="duration">${formatDuration(scenario.duration)}</span>
      </div>
      ${errorHtml}
      ${assertionsHtml}
      ${screenshotsHtml}
    </div>`;
}

function renderSuite(suite: SuiteResult): string {
  const open = suite.status === "fail" ? " open" : "";
  const scenariosHtml = suite.scenarios.map(renderScenario).join("");
  const skipNote = suite.skipReason
    ? `<p class="skip-reason">Skipped: ${escapeHtml(suite.skipReason)}</p>`
    : "";

  return `
    <details class="suite" data-status="${suite.status}"${open}>
      <summary>
        ${statusBadge(suite.status)}
        <strong>${escapeHtml(suite.name)}</strong>
        <span class="dim">(${suite.scenarios.length} scenarios)</span>
      </summary>
      ${skipNote}
      ${scenariosHtml}
    </details>`;
}

/**
 * Generate a self-contained HTML report.
 */
export function writeHtmlReport(result: RunResult, reportDir: string): void {
  mkdirSync(reportDir, { recursive: true });

  const suitesHtml = result.suites.map(renderSuite).join("");

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>E2E Report - ${escapeHtml(result.timestamp)}</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #111; color: #eee; padding: 24px; line-height: 1.5; }
  h1 { font-size: 1.5rem; margin-bottom: 8px; }
  .meta { color: #999; font-size: 0.85rem; margin-bottom: 16px; }
  .summary-bar { display: flex; gap: 16px; margin-bottom: 24px; padding: 16px; background: #1a1a1a; border-radius: 8px; }
  .summary-item { text-align: center; }
  .summary-item .count { font-size: 2rem; font-weight: 700; }
  .summary-item .label { font-size: 0.8rem; color: #999; text-transform: uppercase; }
  .summary-item.pass .count { color: #22c55e; }
  .summary-item.fail .count { color: #ef4444; }
  .summary-item.skip .count { color: #eab308; }
  .filters { margin-bottom: 16px; display: flex; gap: 8px; }
  .filters button { padding: 6px 14px; border: 1px solid #333; border-radius: 4px; background: #222; color: #ccc; cursor: pointer; font-size: 0.85rem; }
  .filters button.active { background: #333; color: #fff; border-color: #555; }
  .badge { display: inline-block; padding: 2px 8px; border-radius: 4px; color: #fff; font-size: 0.75rem; font-weight: 600; text-transform: uppercase; }
  .suite { margin-bottom: 12px; background: #1a1a1a; border-radius: 8px; overflow: hidden; }
  .suite summary { padding: 12px 16px; cursor: pointer; display: flex; align-items: center; gap: 8px; }
  .suite summary:hover { background: #222; }
  .dim { color: #666; }
  .scenario { padding: 8px 16px 8px 32px; border-top: 1px solid #262626; }
  .scenario-header { display: flex; align-items: center; gap: 8px; }
  .duration { color: #666; font-size: 0.85rem; }
  .error { background: #2a1515; color: #f87171; padding: 8px 12px; border-radius: 4px; margin-top: 8px; font-size: 0.85rem; overflow-x: auto; }
  .skip-reason { color: #eab308; padding: 8px 16px; font-size: 0.9rem; }
  .assertions { width: 100%; margin-top: 8px; border-collapse: collapse; font-size: 0.85rem; }
  .assertions th { text-align: left; padding: 4px 8px; border-bottom: 1px solid #333; color: #999; }
  .assertions td { padding: 4px 8px; border-bottom: 1px solid #222; }
  .screenshots { display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap; }
  .thumb { width: 120px; height: auto; border-radius: 4px; border: 1px solid #333; }
  .hidden { display: none !important; }
</style>
</head>
<body>
<h1>Visio E2E Report</h1>
<p class="meta">${escapeHtml(result.timestamp)} &mdash; ${formatDuration(result.duration)}</p>

<div class="summary-bar">
  <div class="summary-item pass"><div class="count">${result.summary.pass}</div><div class="label">Passed</div></div>
  <div class="summary-item fail"><div class="count">${result.summary.fail}</div><div class="label">Failed</div></div>
  <div class="summary-item skip"><div class="count">${result.summary.skip}</div><div class="label">Skipped</div></div>
</div>

<div class="filters">
  <button class="active" data-filter="all">All</button>
  <button data-filter="pass">Pass</button>
  <button data-filter="fail">Fail</button>
  <button data-filter="skip">Skip</button>
</div>

${suitesHtml}

<script>
document.querySelector('.filters').addEventListener('click', (e) => {
  if (e.target.tagName !== 'BUTTON') return;
  const filter = e.target.dataset.filter;
  document.querySelectorAll('.filters button').forEach(b => b.classList.remove('active'));
  e.target.classList.add('active');
  document.querySelectorAll('.suite').forEach(s => {
    if (filter === 'all') { s.classList.remove('hidden'); }
    else { s.classList.toggle('hidden', s.dataset.status !== filter); }
  });
});
</script>
</body>
</html>`;

  writeFileSync(join(reportDir, "report.html"), html, "utf-8");
}
