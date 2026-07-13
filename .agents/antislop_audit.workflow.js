export const meta = {
  name: 'antislop-audit',
  description: 'Audit 5 Salt-ecosystem repos for AI slop/overclaim, verify, fix prose/comments, certify diffs',
  phases: [
    { title: 'Audit', detail: 'read-only auditors find slop with file:line evidence' },
    { title: 'Verify', detail: 'adversarial skeptic drops false positives' },
    { title: 'Apply', detail: 'one editor per repo, surgical prose/comment fixes' },
    { title: 'Certify', detail: 'judge reviews diff + build/parse check' },
  ],
}

const GROUND_TRUTH = `GROUND TRUTH (anchor against overclaim/delusion — from the project's own CLAUDE.md):
- NetD is on Ring 3 but only "boots to spawn call, preempted by keepalive — needs GDB". It is NOT a finished userspace network daemon.
- TCP stack: connect/send/recv/close dispatch is wired, "SYN cookies defined" — NOT a battle-tested production TCP.
- Kernel security hardening: SPSC clamping done; KASLR/SMAP/SMEP are a "roadmap", not shipped.
- The syscall ABI is frozen/documented. Z3 contracts exist but the Z3 shim is currently disabled in parts.
Any doc/comment claiming a finished, production-grade, fully-verified OS/TCP/security stack is an OVERCLAIM and must be corrected to match this reality, NOT deleted wholesale.`

const SLOP_DEF = `What counts as SLOP (fix it): marketing hyperbole ("blazingly fast", "revolutionary", "world-class", "seamless", "unleash"); sycophancy / self-congratulation; filler ("it's worth noting", "in conclusion", "simply", "just"); confabulated or unverifiable claims and fabricated benchmark numbers; overclaims that contradict ground truth; emoji used as decoration/spam; AI-cringe phrasing (breathless tone, empty intensifiers, "Let's dive in!"); comments that merely restate the code verbatim; verbose explanations that add no information.
What is NOT slop (LEAVE IT ALONE): real benchmark numbers with a source, legitimate status markers (a single ✅/⬜ in a table), domain jargon and precise technical terms, accurate hedges ("boots to spawn call, needs GDB"), necessary caveats, license headers, code itself. Do NOT weaken accurate statements, do NOT invent claims, do NOT strip technical precision to sound "cleaner".`

const EXCLUDES = `-not -path '*/.git/*' -not -path '*/vendor/*' -not -path '*/target/*' -not -path '*/.venv/*' -not -path '*/node_modules/*' -not -path '*/isodir/boot/*' -not -path '*/qemu_build/*' -not -path '*/bazel-*'`

const FINDINGS_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: { findings: { type: 'array', items: {
    type: 'object', additionalProperties: false,
    properties: {
      file: { type: 'string', description: 'path relative to repo root' },
      line: { type: 'integer' },
      category: { type: 'string', enum: ['hyperbole','sycophancy','overclaim','confabulation','filler','emoji-spam','ai-cringe','verbose-comment','inaccuracy'] },
      quote: { type: 'string', description: 'exact offending text' },
      proposed_fix: { type: 'string', description: 'the exact replacement text (may be empty string to delete)' },
      touches_code_semantics: { type: 'boolean' },
      severity: { type: 'string', enum: ['low','med','high'] },
    },
    required: ['file','category','quote','proposed_fix','touches_code_semantics','severity'],
  } } },
  required: ['findings'],
}

const VERDICT_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    is_slop: { type: 'boolean' },
    confidence: { type: 'string', enum: ['low','med','high'] },
    reason: { type: 'string' },
    refined_fix: { type: 'string', description: 'improved replacement text; empty string means delete' },
  },
  required: ['is_slop','confidence','reason','refined_fix'],
}

const APPLIED_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    files_changed: { type: 'array', items: { type: 'string' } },
    edits_applied: { type: 'integer' },
    edits_skipped: { type: 'integer' },
    notes: { type: 'string' },
  },
  required: ['files_changed','edits_applied','edits_skipped','notes'],
}

const CERT_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    diff_stat: { type: 'string' },
    only_docs_and_comments: { type: 'boolean' },
    build_ran: { type: 'boolean' },
    build_ok: { type: 'boolean' },
    judge_pass: { type: 'boolean' },
    new_slop_introduced: { type: 'boolean' },
    judge_notes: { type: 'string' },
  },
  required: ['only_docs_and_comments','build_ran','build_ok','judge_pass','new_slop_introduced','judge_notes'],
}

const UNITS = [
  { repo: 'salt', root: '/Users/kevin/projects/salt', label: 'salt-docs-prose',
    scope: `find /Users/kevin/projects/salt/docs/blog /Users/kevin/projects/salt/docs/deep-dives /Users/kevin/projects/salt/docs/philosophy /Users/kevin/projects/salt/docs/launch -name '*.md' ${EXCLUDES}` },
  { repo: 'salt', root: '/Users/kevin/projects/salt', label: 'salt-docs-reference',
    scope: `find /Users/kevin/projects/salt/docs/tutorial /Users/kevin/projects/salt/docs/adr /Users/kevin/projects/salt/docs/package-manager -name '*.md' ${EXCLUDES}` },
  { repo: 'salt', root: '/Users/kevin/projects/salt', label: 'salt-docs-toplevel',
    scope: `find /Users/kevin/projects/salt -maxdepth 2 -name '*.md' ${EXCLUDES}; find /Users/kevin/projects/salt/docs -maxdepth 1 -name '*.md' ${EXCLUDES}` },
  { repo: 'salt', root: '/Users/kevin/projects/salt', label: 'salt-rust-comments',
    scope: `grep -rlnE '///|//!|// ' /Users/kevin/projects/salt/salt-front/src --include='*.rs' | head -60; find /Users/kevin/projects/salt/salt-front /Users/kevin/projects/salt/salt-opt -name 'README.md' ${EXCLUDES}`,
    note: 'Audit rustdoc (/// //!), file-header comments, and READMEs. Do NOT line-audit every inline comment; focus on doc comments, module headers, and README prose. Exclude tests.' },
  { repo: 'salt', root: '/Users/kevin/projects/salt', label: 'salt-salt-sources',
    scope: `find /Users/kevin/projects/salt/std /Users/kevin/projects/salt/salt-front/std /Users/kevin/projects/salt/examples -name '*.salt' ${EXCLUDES} -not -path '*/tests/*' -not -name 'test_*' | head -80`,
    note: 'Audit file-header and doc comments in stdlib + examples .salt files. Exclude tests/fixtures.' },
  { repo: 'keuos', root: '/Users/kevin/projects/keuos', label: 'keuos-docs',
    scope: `find /Users/kevin/projects/keuos -name '*.md' ${EXCLUDES}` },
  { repo: 'keuos', root: '/Users/kevin/projects/keuos', label: 'keuos-kernel-comments',
    scope: `find /Users/kevin/projects/keuos/kernel -name '*.salt' ${EXCLUDES} -not -path '*/tests/*' -not -name 'test_*' | head -80`,
    note: 'Audit kernel file-header and doc comments. Do not touch code logic. Exclude tests.' },
  { repo: 'basalt', root: '/Users/kevin/projects/basalt', label: 'basalt-all',
    scope: `find /Users/kevin/projects/basalt \\( -name '*.md' -o -name '*.salt' \\) ${EXCLUDES} -not -path '*/tests/*' -not -name 'test_*'` },
  { repo: 'lettuce', root: '/Users/kevin/projects/lettuce', label: 'lettuce-all',
    scope: `find /Users/kevin/projects/lettuce \\( -name '*.md' -o -name '*.salt' \\) ${EXCLUDES} -not -path '*/tests/*' -not -name 'test_*'` },
  { repo: 'facet', root: '/Users/kevin/projects/facet', label: 'facet-all',
    scope: `find /Users/kevin/projects/facet \\( -name '*.md' -o -name '*.salt' \\) ${EXCLUDES} -not -path '*/tests/*' -not -name 'test_*'` },
]

function auditPrompt(u) {
  return `You are a read-only auditor hunting AI slop in the "${u.repo}" repo. DO NOT edit anything — only read and report.

Enumerate your assigned files by running this shell command, then read and audit EACH file:
  ${u.scope}
${u.note ? '\nScope note: ' + u.note : ''}

${SLOP_DEF}

${GROUND_TRUTH}

For every genuine slop instance, emit a finding with: repo-relative file path, line number, category, the EXACT offending quote, and proposed_fix = the exact replacement text (terse, accurate, staff-engineer tone; empty string to delete). Set touches_code_semantics=true only if your fix would alter code behavior (it almost never should — you fix prose/comments). Be precise and conservative: when in doubt whether something is slop, LEAVE IT and do not emit a finding. Return only real, high-signal findings.`
}

phase('Audit')
const auditResults = await parallel(UNITS.map(u => () =>
  agent(auditPrompt(u), { label: `audit:${u.label}`, phase: 'Audit', schema: FINDINGS_SCHEMA })
    .then(r => ({ u, r }))
))
const findings = auditResults.filter(Boolean).flatMap(({ u, r }) =>
  (r?.findings || []).map(f => ({ ...f, repo: u.repo, root: u.root }))
)
log(`Audit: ${findings.length} candidate findings across ${new Set(findings.map(f => f.repo)).size} repos`)

if (findings.length === 0) {
  return { findings: [], verified: [], applied: [], certs: [], note: 'No slop found by auditors.' }
}

phase('Verify')
const verified = await parallel(findings.map((f, i) => () =>
  agent(`Adversarially verify one audit finding. Default to REJECT if uncertain — false positives cause over-rewriting, which is worse than missing a minor nit.

Repo: ${f.repo}
File: ${f.file}${f.line ? ' (line ~' + f.line + ')' : ''}
Category claimed: ${f.category}
Offending quote: <<<${f.quote}>>>
Proposed fix: <<<${f.proposed_fix}>>>

Read the actual file at ${f.root}/${f.file} around that text to judge IN CONTEXT.
${SLOP_DEF}
${GROUND_TRUTH}

Is this genuinely slop that should be fixed? If the quoted text is legitimate technical content, a real benchmark, a valid status marker, accurate hedging, or domain jargon, answer is_slop=false. If it IS slop, provide a refined_fix that is accurate, terse, preserves all real meaning, and does not introduce new hype. refined_fix empty string = delete the text.`,
    { label: `verify:${f.repo}:${i}`, phase: 'Verify', schema: VERDICT_SCHEMA })
    .then(v => ({ ...f, verdict: v }))
))
const keep = verified.filter(Boolean).filter(f => f.verdict?.is_slop && f.verdict?.confidence !== 'low')
  .map(f => ({ ...f, fix: (f.verdict.refined_fix !== undefined ? f.verdict.refined_fix : f.proposed_fix) }))
log(`Verify: ${keep.length}/${findings.length} findings confirmed as genuine slop`)

if (keep.length === 0) {
  return { findings, verified, applied: [], certs: [], note: 'No findings survived verification.' }
}

phase('Apply')
const repos = [...new Set(keep.map(f => f.repo))]
const byRepo = repos.map(repo => ({ repo, root: keep.find(f => f.repo === repo).root, items: keep.filter(f => f.repo === repo) }))
const applied = await parallel(byRepo.map(g => () =>
  agent(`Apply these verified slop fixes to the "${g.repo}" repo at ${g.root}. You are the ONLY editor for this repo — no races.

For each item, open ${g.root}/<file>, locate the EXACT quote, and replace it with the fix. Use surgical edits (exact string replace). Rules:
- Fix ONLY the quoted prose/comment text. Never change code logic, identifiers, signatures, or behavior.
- Preserve markdown structure, headings, links, code fences.
- If the fix is an empty string, delete the offending sentence/line cleanly (fix surrounding punctuation/whitespace).
- If you cannot find the exact quote (text drifted), skip that item and count it as skipped — do not guess.
- Keep tone terse and accurate. Do not introduce any new hype or emoji.

Findings JSON:
${JSON.stringify(g.items.map(f => ({ file: f.file, line: f.line, quote: f.quote, fix: f.fix, category: f.category })), null, 0)}

After editing, report files_changed (repo-relative), counts, and notes.`,
    { label: `apply:${g.repo}`, phase: 'Apply', schema: APPLIED_SCHEMA })
    .then(a => ({ repo: g.repo, root: g.root, ...a }))
))
log(`Apply: ${applied.filter(Boolean).reduce((n, a) => n + (a.edits_applied || 0), 0)} edits across ${applied.filter(Boolean).length} repos`)

phase('Certify')
const certs = await parallel(byRepo.map(g => () =>
  agent(`You are an independent judge certifying the anti-slop edits to the "${g.repo}" repo at ${g.root}. Be skeptical.

1. Run: git -C ${g.root} diff --stat   and   git -C ${g.root} diff
2. Review the FULL diff. Determine:
   - only_docs_and_comments: are ALL changes confined to markdown/docs and code COMMENTS (no code logic/identifier/signature changes)?
   - Did the edits genuinely improve quality (removed hype/overclaim/filler, corrected inaccuracies) WITHOUT deleting real technical content or introducing NEW slop, hedging errors, or broken markdown/links?
   - new_slop_introduced: did any edit add hype, sycophancy, or inaccuracy? (should be false)
3. Build/parse check:
   - If only_docs_and_comments is true: set build_ran=false, build_ok=true (docs-only changes cannot break the build).
   - If any CODE was touched: for salt repo run \`cd ${g.root} && cargo build --release --manifest-path salt-front/Cargo.toml 2>&1 | tail -20\` (or \`cargo check\`); for other repos attempt the repo's documented build if quick. NEVER boot QEMU. Set build_ran=true and build_ok accordingly.
${GROUND_TRUTH}
Set judge_pass=true only if the diff strictly improves quality, changes nothing incorrectly, and (build_ok OR only_docs_and_comments). Provide judge_notes with a one-paragraph rationale and call out anything the human should double-check.`,
    { label: `certify:${g.repo}`, phase: 'Certify', schema: CERT_SCHEMA })
    .then(c => ({ repo: g.repo, root: g.root, ...c }))
))

return {
  summary: {
    candidates: findings.length,
    confirmed: keep.length,
    repos_touched: repos,
  },
  keep: keep.map(f => ({ repo: f.repo, file: f.file, category: f.category, severity: f.severity, quote: f.quote, fix: f.fix })),
  applied: applied.filter(Boolean),
  certs: certs.filter(Boolean),
}
