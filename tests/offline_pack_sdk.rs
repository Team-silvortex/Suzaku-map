//! Lightweight SDK authoring guards; schema/decoder rules stay in the Rust validator.
use std::process::Command;

const SDK_CHECKS: &str = r#"
import copy
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sdk_path = Path(sys.argv[1])
spec = importlib.util.spec_from_file_location('suzaku_pack_sdk_guard', sdk_path)
sdk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sdk)


def builder(**overrides):
    metadata = dict(identifier='example.en.collections', language='en',
                    name='Collections', description='Authored SDK guard fixture.',
                    topics=['home'], authors=['Example author'], license='MIT')
    metadata.update(overrides)
    return sdk.PackBuilder(**metadata)


SCALAR_COLLECTIONS = ('hello', '', b'hello', b'', bytearray(b'hello'), bytearray())


class SdkCollectionGuards(unittest.TestCase):
    def test_manifest_collections_reject_scalar_strings_and_bytes_with_field_names(self):
        for field in ('topics', 'authors'):
            for value in SCALAR_COLLECTIONS:
                with self.subTest(field=field, value=value):
                    with self.assertRaisesRegex(ValueError, field):
                        builder(**{field: value})

    def test_word_layer_collections_reject_scalars_without_mutating_the_document(self):
        for field in ('words', 'sentences', 'next_words'):
            for value in SCALAR_COLLECTIONS:
                with self.subTest(field=field, value=value):
                    pack = builder()
                    before = copy.deepcopy(pack.document)
                    arguments = dict(words=['hello'], sentences=['hello there.'],
                                     next_words=[('hello', ['there'])])
                    arguments[field] = value
                    with self.assertRaisesRegex(ValueError, field):
                        pack.words('greeting', **arguments)
                    self.assertEqual(pack.document, before)

    def test_next_word_values_reject_scalar_strings_and_bytes(self):
        for value in SCALAR_COLLECTIONS:
            with self.subTest(value=value):
                pack = builder()
                pack.words('existing', ['hello'])
                before = copy.deepcopy(pack.document)
                with self.assertRaisesRegex(ValueError, re.escape('next_words[0].values')):
                    pack.words('greeting', ['hello'], next_words=[('hello', value)])
                self.assertEqual(pack.document, before)

    def test_next_word_rows_cannot_be_scalar_strings_or_bytes(self):
        for value in SCALAR_COLLECTIONS:
            with self.subTest(value=value):
                pack = builder()
                before = copy.deepcopy(pack.document)
                with self.assertRaisesRegex(ValueError, re.escape('next_words[0]')):
                    pack.words('greeting', ['hello'], next_words=[value])
                self.assertEqual(pack.document, before)

    def test_lists_preserve_strings_and_author_order_as_single_items(self):
        pack = builder(topics=['home', 'family'], authors=['Example author', 'Second author'])
        self.assertIs(pack.words('greeting', ['hello', 'world'],
                                 next_words=[('hello', ['world', 'there'])],
                                 sentences=['hello world.', 'hello there.']), pack)
        self.assertEqual(pack.document['manifest']['topics'], ['home', 'family'])
        self.assertEqual(pack.document['manifest']['authors'], ['Example author', 'Second author'])
        self.assertEqual(pack.document['lexicon']['word_layers'], [{
            'id': 'greeting', 'words': ['hello', 'world'],
            'next_words': [['hello', ['world', 'there']]],
            'sentences': ['hello world.', 'hello there.'],
        }])

    def test_generators_and_tuples_preserve_items_at_every_collection_level(self):
        pack = builder(topics=(x for x in ['home', 'family']),
                       authors=(x for x in ['Example author']))
        row = (x for x in ('hello', (word for word in ('world', 'there'))))
        pack.words('greeting', (word for word in ('hello', 'world')),
                   next_words=(pair for pair in (row,)),
                   sentences=(sentence for sentence in ('hello world.', 'hello there.')))
        self.assertEqual(pack.document['manifest']['topics'], ['home', 'family'])
        self.assertEqual(pack.document['manifest']['authors'], ['Example author'])
        self.assertEqual(pack.document['lexicon']['word_layers'], [{
            'id': 'greeting', 'words': ['hello', 'world'],
            'next_words': [['hello', ['world', 'there']]],
            'sentences': ['hello world.', 'hello there.'],
        }])
        # The normalized authoring document stays ordinary distributable JSON.
        self.assertEqual(json.loads(json.dumps(pack.document)), pack.document)

    def test_none_and_empty_optional_collections_still_work(self):
        for value in (None, [], (), iter(())):
            with self.subTest(value=type(value).__name__):
                pack = builder()
                pack.words('greeting', ['hello'], next_words=value, sentences=value)
                layer = pack.document['lexicon']['word_layers'][0]
                self.assertEqual(layer['next_words'], [])
                self.assertEqual(layer['sentences'], [])

    def test_collection_check_does_not_duplicate_schema_or_decoder_validation(self):
        pack = builder(topics=[], authors=[])
        pack.words('invalid-for-rust', ['two words'], next_words=[['wrong arity']], sentences=[])
        pack.reading('not canonical', 'word')
        self.assertEqual(pack.document['manifest']['topics'], [])
        self.assertEqual(pack.document['lexicon']['word_layers'][0]['words'], ['two words'])
        self.assertEqual(pack.document['lexicon']['word_layers'][0]['next_words'], [['wrong arity']])
        with tempfile.TemporaryDirectory(prefix='suzaku-sdk-authority-') as folder:
            destination = Path(folder) / 'result.json'
            rejected = subprocess.CompletedProcess([], 1, '', 'authoritative decoder rejection')
            with patch.object(sdk.subprocess, 'run', return_value=rejected) as validate:
                with self.assertRaisesRegex(ValueError, 'authoritative decoder rejection'):
                    pack.build(destination, tool='explicit-validator')
                validate.assert_called_once()
            self.assertFalse(destination.exists())
            self.assertEqual(list(Path(folder).iterdir()), [])

    def test_valid_build_calls_validator_then_publishes_without_overwrite(self):
        pack = builder().words('greeting', ['hello'], sentences=['hello there.'])
        expected = (json.dumps(pack.document, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
        with tempfile.TemporaryDirectory(prefix='suzaku-sdk-build-') as folder:
            destination = Path(folder) / 'result.json'

            def validate(arguments, **options):
                self.assertEqual(arguments[:3], ['explicit-validator', 'pack', 'validate'])
                temporary = Path(arguments[3])
                self.assertEqual(temporary.parent, destination.parent)
                self.assertTrue(temporary.name.startswith('.suzaku-pack-'))
                self.assertEqual(temporary.read_bytes(), expected)
                self.assertEqual(options, dict(capture_output=True, text=True, check=False, timeout=30))
                return subprocess.CompletedProcess(arguments, 0, 'valid', '')

            with patch.object(sdk.subprocess, 'run', side_effect=validate) as validate_call:
                self.assertEqual(pack.build(destination, tool='explicit-validator'), destination)
                self.assertEqual(destination.read_bytes(), expected)
                with self.assertRaises(FileExistsError):
                    pack.build(destination, tool='explicit-validator')
                self.assertEqual(validate_call.call_count, 2)
            self.assertEqual(destination.read_bytes(), expected)
            self.assertEqual(list(Path(folder).iterdir()), [destination])

    def test_validator_failure_preserves_existing_destination_and_cleans_temporary_file(self):
        pack = builder().words('greeting', ['hello'])
        with tempfile.TemporaryDirectory(prefix='suzaku-sdk-existing-') as folder:
            destination = Path(folder) / 'result.json'
            destination.write_bytes(b'original author file')
            rejected = subprocess.CompletedProcess([], 1, '', 'validation failed')
            with patch.object(sdk.subprocess, 'run', return_value=rejected):
                with self.assertRaisesRegex(ValueError, 'validation failed'):
                    pack.build(destination, tool='explicit-validator')
            self.assertEqual(destination.read_bytes(), b'original author file')
            self.assertEqual(list(Path(folder).iterdir()), [destination])


unittest.main(argv=['offline-pack-sdk-guards'], verbosity=2)
"#;

#[test]
fn sdk_collection_guards_preserve_authoring_order_and_atomic_publication() {
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sdk/offline-packs/suzaku_pack.py");
    let output = Command::new("python3")
        .args(["-B", "-c", SDK_CHECKS])
        .arg(source)
        .output()
        .expect("run standard-library-only offline pack SDK guards");
    assert!(
        output.status.success(),
        "offline pack SDK guards failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
