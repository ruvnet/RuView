"""Security regressions for the README visual tools' shared XML boundary."""

import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from defusedxml.common import DTDForbidden

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import readme_visuals_xml as svg_xml


class ReadmeVisualXmlTests(unittest.TestCase):
    def test_manifest_paths_reject_absolute_and_traversal_before_resolution(self):
        for relative in [
            '', '/tmp/outside.svg', '//server/share/outside.svg',
            'C:/outside.svg', 'C:outside.svg', r'\\server\share\outside.svg',
            r'\outside.svg', r'\\?\C:\outside.svg', 'file.svg:stream',
            '../outside.svg', 'assets/readme/../../../outside.svg',
            r'assets\readme\..\outside.svg', 'assets/readme/\x00.svg',
        ]:
            with self.subTest(relative=relative):
                with patch.object(Path, 'resolve') as resolve:
                    with self.assertRaises(ValueError):
                        svg_xml.resolve_repo_file(Path('repository'), relative)
                    resolve.assert_not_called()

    def test_manifest_paths_keep_assets_and_targets_in_separate_boundaries(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory).resolve()
            asset = repository / 'assets' / 'readme' / 'ordinary.svg'
            asset.parent.mkdir(parents=True)
            asset.write_bytes(b'<svg/>')
            target = repository / 'README.md'
            target.write_bytes(b'# Readme\n')
            self.assertEqual(svg_xml.resolve_repo_file(repository, 'assets/readme/ordinary.svg', asset=True), asset)
            self.assertEqual(svg_xml.resolve_repo_file(repository, 'README.md'), target)
            with patch.object(Path, 'is_file') as is_file:
                with self.assertRaises(ValueError):
                    svg_xml.resolve_repo_file(repository, 'README.md', asset=True)
                is_file.assert_not_called()

    def test_manifest_paths_reject_symlink_escape_before_file_inspection(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            repository = base / 'repository'
            assets = repository / 'assets' / 'readme'
            assets.mkdir(parents=True)
            outside = base / 'outside.svg'
            outside.write_bytes(b'<svg/>')
            try:
                (assets / 'escape.svg').symlink_to(outside)
                (repository / 'escaped-directory').symlink_to(base, target_is_directory=True)
            except (OSError, NotImplementedError) as error:
                if sys.platform == 'win32' and (isinstance(error, NotImplementedError) or getattr(error, 'winerror', None) == 1314):
                    self.skipTest('Windows symlink privilege unavailable')
                raise
            for relative, asset in [('assets/readme/escape.svg', True), ('escaped-directory/outside.svg', False)]:
                with self.subTest(relative=relative):
                    with patch.object(Path, 'is_file') as is_file:
                        with self.assertRaises(ValueError):
                            svg_xml.resolve_repo_file(repository, relative, asset=asset)
                        is_file.assert_not_called()

    def test_asset_directory_symlink_cannot_redefine_trusted_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            repository = base / 'repository'
            (repository / 'assets').mkdir(parents=True)
            outside = base / 'outside'
            outside.mkdir()
            (outside / 'escape.svg').write_bytes(b'<svg/>')
            try:
                (repository / 'assets' / 'readme').symlink_to(outside, target_is_directory=True)
            except (OSError, NotImplementedError) as error:
                if sys.platform == 'win32' and (isinstance(error, NotImplementedError) or getattr(error, 'winerror', None) == 1314):
                    self.skipTest('Windows symlink privilege unavailable')
                raise
            with patch.object(Path, 'is_file') as is_file:
                with self.assertRaises(ValueError):
                    svg_xml.resolve_repo_file(repository, 'assets/readme/escape.svg', asset=True)
                is_file.assert_not_called()

    def test_plain_utf8_svg_parses(self):
        root = svg_xml.parse_svg('<svg><title>Présence</title></svg>'.encode('utf-8'))
        self.assertEqual(root.tag, 'svg')
        self.assertEqual(root.find('title').text, 'Présence')

    def test_dtd_and_entity_payloads_are_rejected(self):
        for raw in [
            b'<!DOCTYPE svg><svg/>',
            b'<!DOCTYPE svg [<!ENTITY payload "expanded">]><svg>&payload;</svg>',
            b'<!DOCTYPE svg [<!ENTITY payload SYSTEM "file:///private.txt">]><svg>&payload;</svg>',
            b'<!DOCTYPE svg SYSTEM "https://example.invalid/remote.dtd"><svg/>',
        ]:
            with self.subTest(raw=raw):
                with self.assertRaises(DTDForbidden):
                    svg_xml.parse_svg(raw)

    def test_size_limit_is_enforced_before_parser_runs(self):
        for size in [svg_xml.MAX_SVG_BYTES, svg_xml.MAX_SVG_BYTES + 1]:
            with self.subTest(size=size):
                with patch.object(svg_xml.ElementTree, 'fromstring') as parser:
                    with self.assertRaisesRegex(ValueError, 'smaller than 20000 bytes'):
                        svg_xml.parse_svg(b' ' * size)
                    parser.assert_not_called()

    def test_limit_counts_utf8_bytes_and_allows_last_valid_size(self):
        valid = b'<svg/>' + b' ' * (svg_xml.MAX_SVG_BYTES - 7)
        self.assertEqual(len(valid), svg_xml.MAX_SVG_BYTES - 1)
        self.assertEqual(svg_xml.parse_svg(valid).tag, 'svg')
        text = '<svg>' + 'é' * 10000 + '</svg>'
        self.assertLess(len(text), svg_xml.MAX_SVG_BYTES)
        with self.assertRaises(ValueError):
            svg_xml.parse_svg(text.encode('utf-8'))

    def test_reader_never_requests_unbounded_input(self):
        class BoundedReader(io.BytesIO):
            requested = None

            def read(self, size=-1):
                self.requested = size
                if size < 0 or size > svg_xml.MAX_SVG_BYTES:
                    raise AssertionError('unbounded read')
                return super().read(size)

        stream = BoundedReader(b' ' * (svg_xml.MAX_SVG_BYTES * 2))
        with patch.object(Path, 'open', return_value=stream):
            with patch.object(svg_xml.ElementTree, 'fromstring') as parser:
                with self.assertRaises(ValueError):
                    svg_xml.read_svg(Path('oversized.svg'))
                parser.assert_not_called()
        self.assertEqual(stream.requested, svg_xml.MAX_SVG_BYTES)

    def test_file_reader_uses_same_hardened_parser(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'fixture.svg'
            raw = b'<svg><title>ordinary</title></svg>'
            path.write_bytes(raw)
            read, root = svg_xml.read_svg(path)
            self.assertEqual(read, raw)
            self.assertEqual(root.find('title').text, 'ordinary')
            path.write_bytes(b'<!DOCTYPE svg><svg/>')
            with self.assertRaises(DTDForbidden):
                svg_xml.read_svg(path)


if __name__ == '__main__':
    unittest.main()
