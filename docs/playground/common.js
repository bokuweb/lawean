// playground 共通: WASM の読み込み（Rust + Lean）、報告の描画、候補の表
import init, { check, candidates, outline, smt_scripts } from './pkg/lawean_wasm.js';
import createLean from './lean/lawean_lean.js';
export { check, candidates, outline, smt_scripts };

// 溶け込みは Lean の C 出力を Emscripten で組んだ WASM（証明した applyUnit そのもの、ADR-0017）で計算する。
// Rust 側（lawean-leanrt）は globalThis のこの 4 つを呼ぶ。読み込めなければ Rust の写しに戻る（Report.engine で分かる）
let leanEngine = null;
export async function loadLean() {
  try {
    const M = await createLean();
    const rc = M.cwrap('lawean_leanrt_init', 'number', [])();
    if (rc !== 0) throw new Error('lean init failed: ' + rc);
    const wrap = (name) => {
      const f = M.cwrap(name, 'number', ['string', 'string']);
      const free = M.cwrap('lawean_leanrt_free', null, ['number']);
      return (a, b) => {
        try {
          const p = f(a, b); const s = M.UTF8ToString(p); free(p); return s;
        } catch (e) {
          // Lean の WASM が落ちたら（大きな法令で "memory access out of bounds" になることがある）、
          // 以後は Rust の写しに戻す。Rust 側は "ok"/"none" 以外を Err と見て写しで計算し、Report.engine が "rust" になる
          console.warn('Lean wasm failed; falling back to the Rust mirror', e);
          leanEngine = null;
          return 'error: ' + e;
        }
      };
    };
    leanEngine = { apply: wrap('lawean_leanrt_apply_unit'), check: wrap('lawean_leanrt_check_unit'), relation: wrap('lawean_leanrt_relation') };
  } catch (e) {
    console.warn('Lean wasm not available, falling back to the Rust mirror', e);
  }
  globalThis.lawean_lean_available = () => leanEngine !== null;
  globalThis.lawean_lean_apply_unit = (rev, unit) => leanEngine.apply(rev, unit);
  globalThis.lawean_lean_check_unit = (rev, unit) => leanEngine.check(rev, unit);
  globalThis.lawean_lean_relation = (a, b) => leanEngine.relation(a, b);
}

export async function setup() {
  await init();
  await loadLean();
}

export const FIX = '../../fixtures/';
const cache = new Map();
export async function load(rel) {
  if (!cache.has(rel)) cache.set(rel, fetch(FIX + rel).then(r => { if (!r.ok) throw new Error(rel + ': ' + r.status); return r.text(); }));
  return cache.get(rel);
}
export function fill(sel, items, allowNone) {
  sel.innerHTML = '';
  if (allowNone) sel.append(new Option('（無し）', ''));
  for (const [v, t] of items) sel.append(new Option(t, v));
}
export const $ = id => document.getElementById(id);
export function esc(s) { return String(s).replace(/[&<>]/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;'}[c])); }

export const KIND_JA = { parse:'改め文', base:'発射台', order:'施行順序', conflict:'衝突', hane:'ハネ', consolidate:'溶け込み', expected:'改正後との一致', taisho:'新旧対照表', cross_law:'他法令', enforcement:'施行期日', penalty:'罰則' };
export const FIELD_JA = { duration_value:'期間の値', duration:'期間（裸）', period:'事象からの期間', elapsed:'経過', within:'期限', window:'窓', before:'前', within_before:'前の期間', nth_day:'N日目', every:'周期', approx:'目途', compare:'期間の比較', calendar_day:'暦日', era_date:'元号日付', enforcement:'施行期日', sanction:'刑', penalty_target:'罰則の対象規定', penalty_act:'罰則の行為', money:'金額', ratio:'割合', rate:'利率', quantity:'数量' };

// 層 1 の候補（法令の全文から）。原文根拠つきで、値は確定しない
export function candidatesSection(xml, title) {
  let cs;
  try { cs = JSON.parse(candidates(xml)); } catch (e) { return ''; }
  if (!Array.isArray(cs)) return '';
  let h = `<details><summary>${title || '発射台の全文から取った候補'}（層 1、${cs.length} 件: 時間表現・罰則・金額。原文根拠つき、値は確定しない）</summary><table class="cands"><tr><th>種類</th><th>原文</th><th>正規化</th><th>事象・役割</th><th>文脈</th><th>信頼</th></tr>`;
  for (const c of cs.slice(0, 400)) {
    h += `<tr class="${c.confidence}"><td>${FIELD_JA[c.field] || c.field}</td><td>${esc(c.raw)}</td><td>${esc(c.normalized ?? '')}${c.unit ? ' ' + esc(c.unit) : ''}</td><td>${esc(c.source_label ?? '')}${c.role ? ' / ' + esc(c.role) : ''}</td><td class="ctx" title="${esc(c.evidence.sentence)}">${esc(c.evidence.context)}</td><td>${c.confidence}</td></tr>`;
  }
  if (cs.length > 400) h += `<tr><td colspan="6">…ほか ${cs.length - 400} 件</td></tr>`;
  return h + '</table></details>';
}

/// 報告の HTML。`opts.note` は見出しの補足、`opts.extra` は検査の下に足す HTML、`opts.baseXml` があれば候補の表
export function renderReport(r, ms, opts = {}) {
  if (r.error) return `<pre>${esc(r.error)}</pre>`;
  const engine = r.engine === 'lean' ? '溶け込み: Lean（証明した applyUnit の WASM）' : '溶け込み: Rust の写し';
  let h = `<p class="verdict ${r.ok ? 'pass' : 'fail'}">${r.ok ? 'PASS' : 'FAIL'} <span class="status">${ms.toFixed(0)} ms・${engine}${opts.note || ''}</span></p>`;
  h += '<ul class="checks">';
  for (const c of r.checks) {
    const mark = { pass:'✓', fail:'✗', warn:'!', skip:'–' }[c.status];
    h += `<li class="${c.status}"><div class="head"><span class="mark">${mark}</span><span class="kind">${KIND_JA[c.kind] || c.kind}</span><span>${esc(c.message)}</span></div>`;
    if (c.details.length) h += '<ul class="details">' + c.details.map(d => `<li>${esc(d)}</li>`).join('') + '</ul>';
    h += '</li>';
  }
  h += '</ul>';
  if (r.suggested_fixes && r.suggested_fixes.length) h += `<details open><summary>生成したハネの手当て（この改め文を繰り下げの文より前に足す）</summary><pre>${esc(r.suggested_fixes.join('\n'))}</pre></details>`;
  if (opts.extra) h += opts.extra;
  if (r.taisho_generated && r.taisho_generated.length) {
    h += `<details><summary>溶け込みから生成した新旧対照表（${r.taisho_generated.length / 1 | 0} 行。添付資料に。転記ではなく溶け込みから出すので、新旧対照表の誤記は起きない）</summary><pre>${esc(r.taisho_generated.join('\n'))}</pre></details>`;
  }
  if (r.diff.length) {
    h += `<details open><summary>発射台からの差分（${r.diff.length} 項）</summary><pre class="diff">` +
      r.diff.map(d => `<span class="${d.startsWith('追加') ? 'add' : d.startsWith('削除') ? 'del' : 'chg'}">${esc(d)}</span>`).join('\n') + '</pre></details>';
  }
  for (const u of r.units) {
    h += `<details><summary>${esc(u.label)}: ${u.instructions} 文・${u.ops} 操作 → id 操作 ${u.ident_ops.length} 個（Lean の AmendUnit）</summary><pre>${esc(u.ident_ops.join('\n') || '（束縛できなかった）')}</pre></details>`;
  }
  h += `<details><summary>報告の JSON</summary><pre>${esc(JSON.stringify(r, null, 1))}</pre></details>`;
  if (opts.baseXml) h += candidatesSection(opts.baseXml);
  return h;
}
