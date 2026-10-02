#!/usr/bin/env python3
"""Vérifie la séparation stable/RC dans les étapes réellement publiées par la CI."""

import os
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = (ROOT / '.github/workflows/release.yml').read_text()
NOTES = runpy.run_path(str(ROOT / 'scripts/release-notes.py'))
FEED = runpy.run_path(str(ROOT / 'scripts/update-feed.py'))


def resolve_step() -> str:
    tail = WORKFLOW.split('      - id: resolve\n', 1)[1].split('        run: |\n', 1)[1]
    lines = []
    for line in tail.splitlines():
        if line.strip() and not line.startswith('          '):
            break
        lines.append(line[10:] if line.startswith('          ') else '')
    return '\n'.join(lines)


def resolve(source: str, *, tag: str = '', requested: str = '', prerelease: str = ''):
    with tempfile.TemporaryDirectory(prefix='dmx-release-policy-') as temporary:
        root = Path(temporary)
        (root / 'VERSION').write_text(source + '\n')
        output = root / 'outputs'
        environment = os.environ | {
            'REQUESTED_VERSION': requested,
            'REQUESTED_PRERELEASE': prerelease,
            'GITHUB_REF_TYPE': 'tag' if tag else 'branch',
            'GITHUB_REF_NAME': tag or 'main',
            'GITHUB_OUTPUT': str(output),
        }
        process = subprocess.run(
            ['bash', '-euo', 'pipefail', '-c', resolve_step()],
            cwd=root, env=environment, capture_output=True, text=True,
        )
        values = dict(line.split('=', 1) for line in output.read_text().splitlines()) if output.exists() else {}
        return process.returncode, values


class ReleasePolicyTests(unittest.TestCase):
    def test_stable_tag_keeps_stable_feed(self):
        code, output = resolve('2.0.9', tag='v2.0.9')
        self.assertEqual(code, 0)
        self.assertEqual(output, {'version': '2.0.9', 'prerelease': 'false', 'update_feed': 'updates.json'})

    def test_rc_tag_and_manual_release_always_use_rc_feed(self):
        for arguments in ({'tag': 'v2.1.0-rc.1'}, {'requested': '2.1.0-rc.1'}, {}):
            with self.subTest(arguments=arguments):
                code, output = resolve('2.1.0-rc.1', **arguments)
                self.assertEqual(code, 0)
                self.assertEqual(output, {
                    'version': '2.1.0-rc.1', 'prerelease': 'true', 'update_feed': 'updates-rc.json',
                })

    def test_manual_prerelease_does_not_publish_stable_feed(self):
        code, output = resolve('2.1.0', requested='2.1.0', prerelease='true')
        self.assertEqual(code, 0)
        self.assertEqual(output['prerelease'], 'true')
        self.assertEqual(output['update_feed'], 'updates-rc.json')

    def test_mismatch_and_invalid_versions_fail_before_any_output(self):
        for source, tag in (
            ('2.1.0-rc.1', 'v2.0.9'),
            ('2.1.0-rc.1', 'v2.1.0-rc.2'),
            ('3.1.0', 'v3.1.0'),
            ('2.01.0', 'v2.01.0'),
            ('2.1.0-', 'v2.1.0-'),
        ):
            with self.subTest(source=source, tag=tag):
                code, output = resolve(source, tag=tag)
                self.assertNotEqual(code, 0)
                self.assertEqual(output, {})

    def test_prerelease_is_explicitly_excluded_from_latest(self):
        tail = WORKFLOW.split('          flags=()\n', 1)[1].split('          gh release create ', 1)[0]
        block = 'flags=()\n' + '\n'.join(line[10:] for line in tail.splitlines())
        block += '\nif [ ${#flags[@]} -gt 0 ]; then printf "%s\\n" "${flags[@]}"; fi'
        for prerelease, expected in (('true', ['--prerelease', '--latest=false']), ('false', [])):
            with self.subTest(prerelease=prerelease):
                process = subprocess.run(
                    ['bash', '-euo', 'pipefail', '-c', block],
                    env=os.environ | {'PRERELEASE': prerelease}, capture_output=True, text=True, check=True,
                )
                self.assertEqual(process.stdout.splitlines(), expected)
        self.assertIn('PRERELEASE: ${{ needs.version.outputs.prerelease }}', WORKFLOW)

    def test_release_notes_match_the_whole_version_token(self):
        fixture = '''# Changelog
## 2.1.0-rc.10 — 2026-10-02
- RC10 seulement.
## [2.1.0-rc.1] — 2026-10-02
- RC1 seulement.
## 2.1.0 — 2026-10-02
- Stable seulement.
## 2.0.10 — 2026-10-02
- Ancienne version 10.
## 2.0.1 — 2026-10-02
- Ancienne version 1.
'''
        with tempfile.TemporaryDirectory(prefix='dmx-release-notes-') as temporary:
            changelog = Path(temporary) / 'CHANGELOG.md'
            changelog.write_text(fixture)
            for version, expected in (
                ('2.1.0-rc.1', '- RC1 seulement.'),
                ('2.1.0-rc.10', '- RC10 seulement.'),
                ('2.1.0', '- Stable seulement.'),
                ('2.0.1', '- Ancienne version 1.'),
            ):
                with self.subTest(version=version):
                    self.assertEqual(NOTES['notes'](changelog, version), expected)
                    with patch.object(Path, 'read_text', return_value=fixture):
                        self.assertEqual(FEED['release_notes'](version), expected)


if __name__ == '__main__':
    unittest.main()
