import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parents[1] / 'target/debug/herdr-infobox').resolve()
env = {key: value for key, value in os.environ.items() if not key.startswith(('HERDR_', 'INFOBOX_', 'DEVIN_'))}
with tempfile.TemporaryDirectory(prefix='infobox-collector-benchmark-') as directory:
    command = [str(binary), '--state-dir', directory]
    session = subprocess.check_output(command + ['session', 'add', '--provider', 'claude', '--native-id', 'synthetic-benchmark'], env=env).decode().strip()
    samples = []
    for index in range(105):
        event = {'session_id': 'synthetic-benchmark', 'cwd': directory, 'hook_event_name': 'PreToolUse', 'tool_name': 'WebFetch', 'tool_use_id': f'benchmark-{index}', 'tool_input': {'url': f'https://example.test/benchmark/{index}', 'prompt': 'Synthetic benchmark'}}
        payload = json.dumps(event).encode()
        start = time.perf_counter()
        result = subprocess.run(command + ['ingest', '--provider', 'claude'], input=payload, capture_output=True, env=env, timeout=10, check=True)
        elapsed = (time.perf_counter() - start) * 1000
        assert not result.stdout and not result.stderr, (result.stdout, result.stderr)
        if index >= 5:
            samples.append(elapsed)
    display = subprocess.check_output(command + ['ui', '--session', session, '--once'], env=env).decode()
    assert 'References 105' in display, display
    for index in range(105):
        assert f'https://example.test/benchmark/{index}\n' in display, index
    print(json.dumps({'binary': str(binary), 'host': platform.platform(), 'warmup': 5, 'samples': len(samples), 'p50_ms': round(statistics.median(samples), 3), 'p95_ms': round(sorted(samples)[94], 3), 'max_ms': round(max(samples), 3), 'silent_processes': 105, 'persisted_references': 105}, indent=2))
