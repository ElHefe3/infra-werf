"""Read-only Markdown tracker validation; Python 3.11+, standard library only."""
from __future__ import annotations
import argparse
from collections import Counter
import json
from pathlib import Path
import re
from urllib.parse import unquote

META = re.compile(r'<!-- atlas-(scope|feature|ticket):\s*(.*?)\s*-->', re.S)
STATES = {'todo', 'ready', 'in_progress', 'blocked', 'done', 'deferred'}
SECTIONS = ('Outcome and included work', 'Exclusions', 'Implementation and contracts',
            'Acceptance criteria', 'Verification', 'Evidence and closure', 'Stop conditions')


def checklist(body):
    return re.findall(r'^- \[([ xX])\] .+', body, re.M)


def validate(root, overrides=None):
    root = Path(root).resolve()
    overrides = overrides or {}
    paths = set(root.glob('*.md')) | set((root/'agent').rglob('*.md'))
    docs = {p.resolve(): overrides.get(p.resolve(), p.read_text(encoding='utf-8-sig')) for p in paths}
    errors, records = [], {k: {} for k in ('scope', 'feature', 'ticket')}
    for path, body in docs.items():
        matches = list(META.finditer(body))
        for index, match in enumerate(matches):
            kind = match[1]
            try:
                data = json.loads(match[2])
            except json.JSONDecodeError:
                errors.append(f'{path.name}: invalid metadata JSON')
                continue
            ident = data.get('id') if isinstance(data, dict) else None
            pattern = {'scope': r'S\d{2}', 'feature': r'F\d{2}', 'ticket': r'T\d{3}'}[kind]
            if not isinstance(ident, str) or not re.fullmatch(pattern, ident):
                errors.append(f'{path.name}: invalid {kind} ID')
                continue
            if ident in records[kind]:
                errors.append(f'duplicate {kind} {ident}')
                continue
            segment = body[match.end():matches[index+1].start() if index+1 < len(matches) else len(body)]
            records[kind][ident] = (data, path, segment)
            folder = 'scopes' if kind == 'scope' else 'features'
            if path.parent != root/'agent'/folder:
                errors.append(f'{ident}: wrong metadata directory')
            deps = data.get('depends_on')
            if not isinstance(deps, list) or not all(isinstance(v, str) for v in deps):
                errors.append(f'{ident}: invalid dependency list')
                data['depends_on'] = []
            elif len(deps) != len(set(deps)):
                errors.append(f'{ident}: duplicate dependency')

    scopes, features, tickets = (records[k] for k in ('scope', 'feature', 'ticket'))
    ancestry = {}
    for kind, group in records.items():
        def visit(ident, chain):
            if ident in chain:
                errors.append(f'dependency cycle at {ident}')
                return set()
            output = set()
            for dep in group[ident][0]['depends_on']:
                if dep not in group:
                    errors.append(f'{ident}: unknown dependency {dep}')
                    continue
                output.add(dep)
                output.update(visit(dep, chain | {ident}))
            return output
        ancestry[kind] = {ident: visit(ident, set()) for ident in group}

    membership = Counter()
    for sid, (data, path, body) in scopes.items():
        members = data.get('features')
        if not isinstance(members, list) or not all(isinstance(v, str) for v in members):
            errors.append(f'{sid}: invalid feature membership list')
            continue
        for fid in members:
            membership[fid] += 1
            if fid not in features or features[fid][0].get('scope') != sid:
                errors.append(f'{sid}: feature membership mismatch for {fid}')
        if type(data.get('deferred')) is not bool:
            errors.append(f'{sid}: deferred must be boolean')
        if data.get('window') not in ({None} if data.get('deferred') else {1, 2}):
            errors.append(f'{sid}: invalid delivery window')
        if not checklist(body):
            errors.append(f'{sid}: missing acceptance gate')
    for fid, (data, path, _) in features.items():
        sid = data.get('scope')
        if sid not in scopes or membership[fid] != 1:
            errors.append(f'{fid}: must belong to exactly one existing scope')
            continue
        if data.get('deferred') != scopes[sid][0].get('deferred'):
            errors.append(f'{fid}: deferred state differs from scope')
        for dep in data['depends_on']:
            dep_scope = features.get(dep, ({},))[0].get('scope')
            if dep_scope and dep_scope != sid and dep_scope not in ancestry['scope'][sid]:
                errors.append(f'{fid}: undeclared prerequisite scope for {dep}')
        if not any(t[0].get('feature') == fid for t in tickets.values()):
            errors.append(f'{fid}: missing tickets')

    def scope_done(sid):
        members = [t for t in tickets.values() if features.get(t[0].get('feature'), ({},))[0].get('scope') == sid]
        gate = checklist(scopes[sid][2])
        return bool(members and gate) and all(t[0].get('status') == 'done' for t in members) and all(c.lower() == 'x' for c in gate)

    def evidence(data, path, ident):
        ref = data.get('evidence')
        if not isinstance(ref, str) or not ref:
            errors.append(f'{ident}: missing evidence')
            return
        target = (path.parent/ref.split('#', 1)[0]).resolve()
        if not target.is_relative_to(root/'agent'/'evidence') or target.suffix != '.md' or target not in docs:
            errors.append(f'{ident}: evidence file missing or outside evidence directory')
            return
        text = docs[target]
        if ident not in text or not re.search(r'^Revision:\s*\S+', text, re.M) or not re.search(r'^Result:\s*pass\s*$', text, re.M | re.I):
            errors.append(f'{ident}: evidence requires ID, tested Revision and Result: pass')

    for tid, (data, path, body) in tickets.items():
        fid = data.get('feature')
        if fid not in features or path != features[fid][1]:
            errors.append(f'{tid}: wrong feature packet')
            continue
        feature = features[fid][0]
        state = data.get('status')
        if state not in STATES:
            errors.append(f'{tid}: invalid status')
        if feature.get('deferred') and state != 'deferred':
            errors.append(f'{tid}: deferred scope must be explicitly activated')
        if not feature.get('deferred') and state == 'deferred':
            errors.append(f'{tid}: core ticket cannot be silently deferred')
        for name in SECTIONS:
            if f'### {name}\n' not in body:
                errors.append(f'{tid}: missing section {name}')
        section = re.search(r'### Acceptance criteria\n(.*?)(?=\n### |\Z)', body, re.S)
        criteria = checklist(section[1]) if section else []
        if not criteria:
            errors.append(f'{tid}: missing acceptance checklist')
        for dep in data['depends_on']:
            dep_feature = tickets.get(dep, ({},))[0].get('feature')
            if dep_feature and dep_feature != fid and dep_feature not in ancestry['feature'][fid]:
                errors.append(f'{tid}: undeclared prerequisite feature for {dep}')
        scope_deps = scopes.get(feature.get('scope'), ({},))[0].get('depends_on', [])
        entry = data.get('scope_entry', {})
        if not isinstance(entry, dict):
            errors.append(f'{tid}: invalid scoped entry')
            entry = {}
        valid_entry = {}
        for sid, required in entry.items():
            if sid not in scope_deps:
                errors.append(f'{tid}: undeclared scoped entry {sid}')
            elif not isinstance(required, list) or not required or any(not isinstance(t, str) for t in required) or len(required) != len(set(required)):
                errors.append(f'{tid}: invalid scoped entry {sid}')
            elif any(t not in ancestry['ticket'][tid] or features.get(tickets.get(t, ({},))[0].get('feature'), ({},))[0].get('scope') != sid for t in required):
                errors.append(f'{tid}: scoped entry must name prerequisite tickets from {sid}')
            else:
                valid_entry[sid] = required
        if state in {'ready', 'in_progress', 'done'}:
            for dep in data['depends_on']:
                if tickets.get(dep, ({},))[0].get('status') != 'done':
                    errors.append(f'{tid}: incomplete dependency {dep}')
            for sid in scope_deps:
                if sid in scopes and not scope_done(sid):
                    if sid not in valid_entry:
                        errors.append(f'{tid}: prerequisite scope {sid} has not passed')
                    elif any(tickets[t][0].get('status') != 'done' for t in valid_entry[sid]):
                        errors.append(f'{tid}: scoped prerequisite {sid} has not passed')
        if state == 'done':
            if any(c.lower() != 'x' for c in criteria):
                errors.append(f'{tid}: unchecked acceptance on done ticket')
            evidence(data, path, tid)
    for sid, (data, path, body) in scopes.items():
        members = [t for t in tickets.values() if features.get(t[0].get('feature'), ({},))[0].get('scope') == sid]
        if members and all(t[0].get('status') == 'done' for t in members):
            if not scope_done(sid):
                errors.append(f'{sid}: completed tickets with unchecked gate')
            evidence(data, path, sid)

    for path, raw in docs.items():
        body = re.sub(r'```.*?```', '', raw, flags=re.S)
        for target in re.findall(r'\[[^\]\n]+\]\(([^)\n]+)\)', body):
            target = unquote(target.strip().strip('<>'))
            if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:', target):
                continue
            dest, _, anchor = target.partition('#')
            resolved = (path.parent/dest).resolve() if dest else path
            if not resolved.is_file():
                errors.append(f'{path.name}: broken link {target}')
            elif anchor and resolved in docs:
                slugs = [re.sub(r'[^\w\s-]', '', h.lower()).replace(' ', '-') for h in re.findall(r'^#+\s+(.+)$', docs[resolved], re.M)]
                if anchor not in slugs:
                    errors.append(f'{path.name}: broken anchor {target}')
    counts = (len(tickets), len(features), len(scopes), sum(not features.get(t[0].get('feature'), ({},))[0].get('deferred', True) for t in tickets.values()))
    counts += (counts[0]-counts[3],)
    guide = docs.get(root/'agent'/'AGENT.md', '')
    total = re.search(r'\*\*Tracker:\*\* (\d+) tickets / (\d+) features / (\d+) scopes: (\d+) core and (\d+) deferred', guide)
    if not total or tuple(map(int, total.groups())) != counts:
        errors.append('roadmap totals disagree with packets')
    return sorted(set(errors)), counts, Counter(t[0].get('status') for t in tickets.values())


def self_test(root):
    root = Path(root).resolve()
    f01 = next((root/'agent'/'features').glob('F01-*.md'))
    f13 = next((root/'agent'/'features').glob('F13-*.md'))
    f02 = next((root/'agent'/'features').glob('F02-*.md'))
    f03 = next((root/'agent'/'features').glob('F03-*.md'))
    scope = next((root/'agent'/'scopes').glob('S01-*.md'))

    # Rejection fixtures need controlled preconditions. They must not stop
    # testing a rule merely because the live dependency has since been done.
    packets = {p: p.read_text(encoding='utf-8-sig') for p in (root/'agent'/'features').glob('*.md')}
    deferred = {json.loads(m[2])['id']: json.loads(m[2])['deferred']
                for text in packets.values() for m in META.finditer(text) if m[1] == 'feature'}

    def reset_ticket(match):
        data = json.loads(match[2])
        if match[1] == 'ticket':
            data['status'] = 'deferred' if deferred[data['feature']] else 'todo'
            if data['id'] == 'T001':
                data['status'] = 'ready'
            data['evidence'] = None
        return '<!-- atlas-'+match[1]+': '+json.dumps(data)+' -->'

    baseline = {p: META.sub(reset_ticket, text).replace('- [x]', '- [ ]').replace('- [X]', '- [ ]')
                for p, text in packets.items()}

    def change(path, kind, ident, edits):
        def replace(match):
            data = json.loads(match[2])
            if match[1] == kind and data['id'] == ident:
                data.update(edits)
            return '<!-- atlas-'+match[1]+': '+json.dumps(data)+' -->'
        return {**baseline, path: META.sub(replace, baseline.get(path, path.read_text(encoding='utf-8-sig')))}

    cases = [
        ('unknown dependency', change(f01, 'ticket', 'T001', {'depends_on': ['T999']})),
        ('dependency cycle', change(f01, 'ticket', 'T001', {'depends_on': ['T002']})),
        ('incomplete dependency', change(f01, 'ticket', 'T002', {'status': 'ready'})),
        ('explicitly activated', change(f13, 'ticket', 'T027', {'status': 'ready'})),
        ('unchecked acceptance', change(f01, 'ticket', 'T001', {'status': 'done'})),
        ('evidence file missing', change(f01, 'ticket', 'T001', {'status': 'done', 'evidence': '../evidence/missing.md'})),
        ('membership mismatch', change(scope, 'scope', 'S01', {'features': ['F01', 'F03']})),
        ('broken link', {**baseline, f01: baseline[f01]+'\n[bad](missing.md)\n'}),
        ('missing section', {**baseline, f01: baseline[f01].replace('### Verification', '### Missing proof', 1)}),
        ('duplicate ticket', {**baseline, f01: baseline[f01]+'\n<!-- atlas-ticket: {"id":"T001","feature":"F01","status":"todo","depends_on":[]} -->\n'}),
        ('invalid scoped entry', change(f03, 'ticket', 'T006', {'scope_entry': {'S01': []}})),
        ('undeclared scoped entry', change(f03, 'ticket', 'T006', {'scope_entry': {'S08': ['T030']}})),
        ('scoped entry must name prerequisite', change(f03, 'ticket', 'T006', {'scope_entry': {'S01': ['T009']}})),
        ('scoped prerequisite', change(f03, 'ticket', 'T006', {'status': 'ready', 'scope_entry': {'S01': ['T003']}})),
    ]
    for expected, overrides in cases:
        errors, _, _ = validate(root, overrides)
        if not any(expected in error for error in errors):
            raise AssertionError(f'Rejection fixture failed: {expected}: {errors}')
    # A valid completion must be possible too; the proof is synthetic/in-memory.
    proof = root/'agent'/'evidence'/'DOCS-001.md'
    valid = change(f01, 'ticket', 'T001', {'status': 'done', 'evidence': '../evidence/DOCS-001.md'})
    before, after = valid[f01].split('## T002', 1)
    valid[f01] = before.replace('- [ ]', '- [x]')+'## T002'+after
    valid[proof] = '# Synthetic T001 proof\nRevision: fixture-only\nResult: pass\n'
    errors, _, _ = validate(root, valid)
    if errors:
        raise AssertionError(f'Valid completion fixture was rejected: {errors}')
    # An explicit phase can enter on its proven ticket prerequisites without
    # pretending the whole prerequisite scope (including phone work) is done.
    phase = {**baseline, proof: '# Synthetic T003 T004 T005 proof\nRevision: fixture-only\nResult: pass\n'}
    def phase_metadata(match):
        data = json.loads(match[2])
        if match[1] == 'ticket' and data['id'] in {'T003', 'T004', 'T005'}:
            data.update(status='done', depends_on=[], evidence='../evidence/DOCS-001.md')
        if match[1] == 'ticket' and data['id'] == 'T006':
            data.update(status='ready', scope_entry={'S01': ['T003', 'T004', 'T005']})
        return '<!-- atlas-'+match[1]+': '+json.dumps(data)+' -->'
    phase[f02] = META.sub(phase_metadata, baseline[f02]).replace('- [ ]', '- [x]')
    phase[f03] = META.sub(phase_metadata, baseline[f03])
    errors, _, _ = validate(root, phase)
    if errors:
        raise AssertionError(f'Valid phased entry fixture was rejected: {errors}')
    default = {**phase, f03: phase[f03].replace(', "scope_entry": {"S01": ["T003", "T004", "T005"]}', '')}
    errors, _, _ = validate(root, default)
    if not any('prerequisite scope S01 has not passed' in e for e in errors):
        raise AssertionError('Default scope gate was silently weakened')
    return len(cases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    errors, counts, states = validate(args.root)
    if errors:
        print('\n'.join('ERROR: '+error for error in errors))
        return 1
    print('Tracker valid: %s tickets, %s features, %s scopes; %s core, %s deferred' % counts)
    print('States:', ', '.join(f'{k}={v}' for k, v in sorted(states.items())))
    if args.self_test:
        print('In-memory rejection fixtures passed:', self_test(args.root))
        print('In-memory positive completion/phased-entry fixtures passed: 2; default gate preserved')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
